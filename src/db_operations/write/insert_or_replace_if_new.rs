use crate::app::DbNamespace;
use std::sync::Arc;

use my_no_sql_sdk::core::db::DbRow;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::DbOperationError,
    db_sync::{states::UpdateRowsSyncData, EventSource, SyncEvent},
};

use super::WriteOperationResult;

/// Insert-or-replace a single row, but only when it is "new":
/// - there is no row with such PartitionKey/RowKey in the table yet, or
/// - the incoming row's `TimeStamp` is strictly greater than the `TimeStamp` of the
///   row already stored.
///
/// When the incoming row is not newer than the stored one, nothing is written and
/// [`WriteOperationResult::Empty`] is returned.
pub async fn execute(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: Arc<DbTable>,
    db_row: Arc<DbRow>,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
    now: DateTimeAsMicroseconds,
) -> Result<WriteOperationResult, DbOperationError> {
    super::super::check_app_states(app)?;

    let result = {
        let mut table_data = db_table.data.write();

        let is_new = match table_data.get_partition(db_row.get_partition_key()) {
            Some(db_partition) => match db_partition.get_row(db_row.get_row_key()) {
                Some(existing) => {
                    db_row.get_time_stamp_as_date_time().unix_microseconds
                        > existing.get_time_stamp_as_date_time().unix_microseconds
                }
                None => true,
            },
            None => true,
        };

        if is_new {
            let (partition_key, _) = table_data.insert_or_replace_row(db_row.clone(), Some(now));

            let mut update_rows_state = UpdateRowsSyncData::new(&table_data, event_src);
            update_rows_state
                .rows_by_partition
                .add_row(partition_key.clone(), db_row.clone());

            Some((partition_key, update_rows_state))
        } else {
            None
        }
    };

    let (partition_key, update_rows_state) = match result {
        Some(result) => result,
        None => return Ok(WriteOperationResult::Empty),
    };

    db_namespace
        .persist_markers
        .persist_rows(
            &db_table.name,
            &partition_key,
            persist_moment,
            [&db_row].into_iter(),
        )
        .await;

    crate::operations::sync::dispatch(app, db_namespace, SyncEvent::UpdateRows(update_rows_state));

    Ok(WriteOperationResult::SingleRow(db_row))
}
