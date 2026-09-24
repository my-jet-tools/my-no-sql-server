use crate::app::DbNamespace;
use std::sync::Arc;

use my_no_sql_sdk::core::db::PartitionKeyParameter;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::DbOperationError,
    db_sync::{states::InitPartitionsSyncEventData, EventSource, SyncEvent},
};

pub async fn delete_partitions(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: &Arc<DbTable>,
    partition_keys: impl Iterator<Item = impl PartitionKeyParameter>,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
    now: DateTimeAsMicroseconds,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app)?;

    let (sync_data, removed_partition_keys) = {
        let mut table_write_access = db_table.data.write();
        let mut sync_data = InitPartitionsSyncEventData::new(&table_write_access, event_src);
        let mut removed_partition_keys = Vec::new();

        for partition_key in partition_keys {
            let remove_partition_result =
                table_write_access.remove_partition(&partition_key, Some(now));

            if let Some(removed_partition) = remove_partition_result {
                removed_partition_keys.push(removed_partition.partition_key.clone());
                sync_data.add(partition_key.into_partition_key(), None);
            }
        }

        (sync_data, removed_partition_keys)
    };

    for partition_key in removed_partition_keys {
        db_namespace
            .persist_markers
            .persist_partition(&db_table.name, &partition_key, persist_moment)
            .await;
    }

    crate::operations::sync::dispatch(app, db_namespace, SyncEvent::InitPartitions(sync_data));

    Ok(())
}
