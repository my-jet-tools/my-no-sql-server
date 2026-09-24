use my_no_sql_sdk::core::db::{DbTableName, PartitionKey};

use std::sync::Arc;

use crate::app::DbNamespace;

pub async fn save_partition(
    db_namespace: &Arc<DbNamespace>,
    db_table_name: &DbTableName,
    partition_key: PartitionKey,
) {
    let db_table = match db_namespace.db.get_table(db_table_name.as_str()) {
        Some(db_table) => db_table,
        None => {
            super::scripts::delete_table(db_namespace, db_table_name).await;
            return;
        }
    };

    match db_table.get_partition_snapshot(partition_key.as_str()) {
        Some(snapshot) => {
            super::scripts::sync_partition_snapshot(
                db_namespace,
                db_table_name,
                &partition_key,
                snapshot,
            )
            .await;
        }
        None => {
            super::scripts::delete_partition(db_namespace, db_table_name, &partition_key).await;
            return;
        }
    };
}
