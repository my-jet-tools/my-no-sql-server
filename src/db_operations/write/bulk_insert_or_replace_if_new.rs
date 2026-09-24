use crate::app::DbNamespace;
use std::sync::Arc;

use my_no_sql_sdk::core::db::{DbRow, PartitionKeyParameter};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::DbOperationError,
    db_sync::{states::UpdateRowsSyncData, EventSource, SyncEvent},
};

/// Bulk insert-or-replace, but a row is written only when it is "new":
/// - there is no row with such PartitionKey/RowKey in the table yet, or
/// - the incoming row's `TimeStamp` is strictly greater than the `TimeStamp` of the
///   row already stored.
///
/// Incoming rows are expected to keep their own `TimeStamp` (parsed via
/// `parse_grouped_by_partition_key_and_keep_date_time`), so the comparison is against
/// the value the client sent, not the server write moment.
pub async fn execute(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: &Arc<DbTable>,
    rows_by_partition: Vec<(impl PartitionKeyParameter, Vec<Arc<DbRow>>)>,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
    now: DateTimeAsMicroseconds,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app)?;

    let (update_rows_state, to_persist, has_writes) = {
        let mut table_data = db_table.data.write();
        let mut update_rows_state = UpdateRowsSyncData::new(&table_data, event_src);
        let mut to_persist: Vec<(my_no_sql_sdk::core::db::PartitionKey, Vec<Arc<DbRow>>)> =
            Vec::new();
        let mut has_writes = false;

        for (partition_key, db_rows) in rows_by_partition {
            let rows_to_write: Vec<Arc<DbRow>> =
                match table_data.get_partition(partition_key.as_str()) {
                    Some(db_partition) => db_rows
                        .into_iter()
                        .filter(|db_row| match db_partition.get_row(db_row.get_row_key()) {
                            Some(existing) => {
                                db_row.get_time_stamp_as_date_time().unix_microseconds
                                    > existing.get_time_stamp_as_date_time().unix_microseconds
                            }
                            None => true,
                        })
                        .collect(),
                    None => db_rows,
                };

            if rows_to_write.is_empty() {
                continue;
            }

            let (partition_key, _) =
                table_data.bulk_insert_or_replace(&partition_key, &rows_to_write, Some(now));

            has_writes = true;
            to_persist.push((partition_key.clone(), rows_to_write.clone()));

            update_rows_state
                .rows_by_partition
                .add_rows(partition_key, rows_to_write);
        }

        (update_rows_state, to_persist, has_writes)
    };

    for (partition_key, db_rows) in to_persist {
        db_namespace
            .persist_markers
            .persist_rows(
                &db_table.name,
                &partition_key,
                persist_moment,
                db_rows.iter(),
            )
            .await;
    }

    if has_writes {
        crate::operations::sync::dispatch(
            app,
            db_namespace,
            SyncEvent::UpdateRows(update_rows_state),
        );
    }

    Ok(())
}
