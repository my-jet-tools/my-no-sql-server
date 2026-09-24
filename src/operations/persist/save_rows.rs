use my_no_sql_sdk::core::db::DbTableName;

use std::sync::Arc;

use crate::{app::DbNamespace, persist_markers::SyncRowJobDescription};

/// Handles a `SyncRows` task. With per-partition storage there is no row-level
/// write: every partition touched by the dirty rows is re-serialized whole
/// (deleted rows simply fall out of the fresh snapshot), or its blob is removed
/// if the partition no longer exists.
pub async fn save_rows(
    db_namespace: &Arc<DbNamespace>,
    db_table_name: &DbTableName,
    jobs: Vec<SyncRowJobDescription>,
) {
    let db_table = match db_namespace.db.get_table(db_table_name.as_str()) {
        Some(db_table) => db_table,
        None => {
            super::scripts::delete_table(db_namespace, db_table_name).await;
            return;
        }
    };

    for job in jobs {
        match db_table.get_partition_snapshot(job.partition_key.as_str()) {
            Some(snapshot) => {
                super::scripts::sync_partition_snapshot(
                    db_namespace,
                    db_table_name,
                    &job.partition_key,
                    snapshot,
                )
                .await;
            }
            None => {
                super::scripts::delete_partition(db_namespace, db_table_name, &job.partition_key)
                    .await;
            }
        }
    }
}
