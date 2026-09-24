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

    let (update_rows_state, to_persist, has_insert_or_replace) = {
        let mut table_data = db_table.data.write();
        let mut update_rows_state = UpdateRowsSyncData::new(&table_data, event_src);
        let mut to_persist: Vec<(my_no_sql_sdk::core::db::PartitionKey, Vec<Arc<DbRow>>)> =
            Vec::new();
        let mut has_insert_or_replace = false;

        for (partition_key, db_rows) in rows_by_partition {
            let (partition_key, _) =
                table_data.bulk_insert_or_replace(&partition_key, &db_rows, Some(now));

            has_insert_or_replace = true;
            to_persist.push((partition_key.clone(), db_rows.clone()));

            update_rows_state
                .rows_by_partition
                .add_rows(partition_key, db_rows);
        }

        (update_rows_state, to_persist, has_insert_or_replace)
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

    if has_insert_or_replace {
        crate::operations::sync::dispatch(
            app,
            db_namespace,
            SyncEvent::UpdateRows(update_rows_state),
        );
    }

    Ok(())
}
