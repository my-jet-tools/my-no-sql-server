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

pub async fn bulk_delete(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: &DbTable,
    rows_to_delete: impl Iterator<Item = (impl PartitionKeyParameter, Vec<impl RowKeyParameter>)>,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
    now: DateTimeAsMicroseconds,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app)?;

    enum PersistOp {
        Partition(my_no_sql_sdk::core::db::PartitionKey),
        Rows(
            my_no_sql_sdk::core::db::PartitionKey,
            Vec<Arc<my_no_sql_sdk::core::db::DbRow>>,
        ),
    }

    let (sync_data, persist_ops) = {
        let mut table_data = db_table.data.write();
        let mut sync_data = DeleteRowsEventSyncData::new(&table_data, event_src);
        let mut persist_ops: Vec<PersistOp> = Vec::new();

        for (partition_key, row_keys) in rows_to_delete {
            let removed_rows_result =
                table_data.bulk_remove_rows(&partition_key, row_keys.into_iter(), true, Some(now));

            if let Some((partition_key, removed_rows, partition_is_empty)) = removed_rows_result {
                if partition_is_empty {
                    sync_data.new_deleted_partition(&partition_key);
                    persist_ops.push(PersistOp::Partition(partition_key));
                } else {
                    sync_data.add_deleted_rows(&partition_key, &removed_rows);
                    persist_ops.push(PersistOp::Rows(partition_key, removed_rows));
                }
            }
        }

        (sync_data, persist_ops)
    };

    for op in persist_ops {
        match op {
            PersistOp::Partition(pk) => {
                db_namespace
                    .persist_markers
                    .persist_partition(&db_table.name, &pk, persist_moment)
                    .await;
            }
            PersistOp::Rows(pk, rows) => {
                db_namespace
                    .persist_markers
                    .delete_db_rows(&db_table.name, &pk, persist_moment, rows.iter())
                    .await;
            }
        }
    }

    crate::operations::sync::dispatch(app, db_namespace, SyncEvent::DeleteRows(sync_data));

    Ok(())
}
