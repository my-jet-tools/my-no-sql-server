use crate::app::DbNamespace;
use std::sync::Arc;

use my_no_sql_sdk::core::db::{DbRow, PartitionKeyParameter};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::DbOperationError,
    db_sync::{states::InitPartitionsSyncEventData, EventSource, SyncEvent},
};

pub async fn clean_partition_and_bulk_insert(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: &Arc<DbTable>,
    partition_to_clean: String,
    entities: Vec<(String, Vec<Arc<DbRow>>)>,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
    now: DateTimeAsMicroseconds,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app)?;

    let (partition_keys, sync_events) = {
        let mut table_data = db_table.data.write();

        table_data.remove_partition(&partition_to_clean, None);

        let mut partition_keys = Vec::new();

        for (partition_key, db_rows) in entities {
            let (partition_key, _) =
                table_data.bulk_insert_or_replace(&partition_key, &db_rows, Some(now));

            partition_keys.push(partition_key);
        }

        let sync_events: Vec<_> = partition_keys
            .iter()
            .map(|pk| {
                InitPartitionsSyncEventData::new_as_update_partition(
                    &table_data,
                    pk.clone().into(),
                    event_src.clone(),
                )
            })
            .collect();

        (partition_keys, sync_events)
    };

    for partition_key in &partition_keys {
        db_namespace
            .persist_markers
            .persist_partition(&db_table.name, partition_key, persist_moment)
            .await;
    }

    // Mark the cleaned partition too: if it was not re-inserted, the persist
    // loop sees an empty snapshot and deletes its on-disk blob (otherwise it is
    // re-saved). Without this its persisted blob would resurrect on restart.
    let cleaned_partition_key = partition_to_clean.into_partition_key();
    db_namespace
        .persist_markers
        .persist_partition(&db_table.name, &cleaned_partition_key, persist_moment)
        .await;

    for state in sync_events {
        crate::operations::sync::dispatch(app, db_namespace, SyncEvent::InitPartitions(state));
    }

    Ok(())
}
