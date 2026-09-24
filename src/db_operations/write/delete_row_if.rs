use crate::app::DbNamespace;
use std::sync::Arc;

use my_no_sql_sdk::core::db::{PartitionKeyParameter, RowKeyParameter};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::DbOperationError,
    db_sync::{states::DeleteRowsEventSyncData, EventSource, SyncEvent},
};

use super::WriteOperationResult;

/// Delete guarded by optimistic concurrency: the row goes only when it is still the
/// version the client read it at. `expected_time_stamp` is that version, parsed out of
/// the `TimeStamp` the client sends back.
///
/// A row which is not there answers `RecordNotFound` (404) and one which has been
/// rewritten meanwhile answers `OptimisticConcurrencyUpdateFails` (409) - the same two
/// answers `replace` gives. The bulk counterpart `bulk_delete_if` reports those two
/// situations per row instead of failing the whole request.
///
/// Versions are compared as parsed moments (microseconds since epoch), never as text,
/// so how many fractional digits the `TimeStamp` was spelled with does not matter.
pub async fn execute(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: &Arc<DbTable>,
    partition_key: impl PartitionKeyParameter,
    row_key: impl RowKeyParameter,
    expected_time_stamp: DateTimeAsMicroseconds,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
    now: DateTimeAsMicroseconds,
) -> Result<WriteOperationResult, DbOperationError> {
    super::super::check_app_states(app)?;

    let (partition_key, removed_row, sync_data) = {
        let mut table_data = db_table.data.write();

        // The version check and the removal happen under the same write lock: checking
        // under a read lock first would leave a window for somebody to rewrite the row
        // in between.
        {
            let Some(db_partition) = table_data.get_partition(partition_key.as_str()) else {
                return Err(DbOperationError::RecordNotFound);
            };

            let Some(db_row) = db_partition.get_row(row_key.as_str()) else {
                return Err(DbOperationError::RecordNotFound);
            };

            if db_row.get_time_stamp_as_date_time().unix_microseconds
                != expected_time_stamp.unix_microseconds
            {
                return Err(DbOperationError::OptimisticConcurrencyUpdateFails);
            }
        }

        // The row was found a couple of statements ago under this very lock, so the
        // removal can not come back empty.
        let (partition_key, removed_row, partition_is_empty) = table_data
            .remove_row(&partition_key, &row_key, true, Some(now))
            .unwrap();

        let mut sync_data = DeleteRowsEventSyncData::new(&table_data, event_src);

        if partition_is_empty {
            sync_data.new_deleted_partition(&partition_key);
        } else {
            sync_data.add_deleted_row(&partition_key, removed_row.clone())
        }

        (partition_key, removed_row, sync_data)
    };

    db_namespace
        .persist_markers
        .persist_rows(
            &db_table.name,
            &partition_key,
            persist_moment,
            [&removed_row].into_iter(),
        )
        .await;

    crate::operations::sync::dispatch(app, db_namespace, SyncEvent::DeleteRows(sync_data));

    Ok(WriteOperationResult::SingleRow(removed_row))
}
