use crate::app::DbNamespace;
use std::sync::Arc;

use my_no_sql_sdk::core::db::DbRow;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::DbOperationError,
    db_sync::{states::InitTableEventSyncData, EventSource, SyncEvent},
};

pub async fn execute(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: Arc<DbTable>,
    entities: Vec<(String, Vec<Arc<DbRow>>)>,
    event_src: Option<EventSource>,
    persist_moment: DateTimeAsMicroseconds,
    now: DateTimeAsMicroseconds,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app)?;

    let sync_data = {
        let mut table_data = db_table.data.write();

        table_data.clear_table();

        for (partition_key, db_rows) in entities {
            table_data.bulk_insert_or_replace(&partition_key, &db_rows, Some(now));
        }

        event_src.map(|event_src| InitTableEventSyncData::new(&table_data, event_src))
    };

    db_namespace
        .persist_markers
        .persist_table_content(&db_table.name, persist_moment)
        .await;

    if let Some(sync_data) = sync_data {
        crate::operations::sync::dispatch(app, db_namespace, SyncEvent::InitTable(sync_data));
    }

    Ok(())
}
