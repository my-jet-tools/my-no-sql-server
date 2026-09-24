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

pub async fn execute(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: &Arc<DbTable>,
    partition_key: impl PartitionKeyParameter,
    row_key: impl RowKeyParameter,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
    now: DateTimeAsMicroseconds,
) -> Result<WriteOperationResult, DbOperationError> {
    super::super::check_app_states(app)?;

    let (partition_key, removed_row, sync_data) = {
        let mut table_data = db_table.data.write();

        let remove_row_result = table_data.remove_row(&partition_key, &row_key, true, Some(now));

        if remove_row_result.is_none() {
            return Ok(WriteOperationResult::Empty);
        }

        let (partition_key, removed_row, partition_is_empty) = remove_row_result.unwrap();

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

    let result = WriteOperationResult::SingleRow(removed_row).into();
    Ok(result)
}
