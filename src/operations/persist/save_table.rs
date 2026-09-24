use my_no_sql_sdk::core::db::DbTableName;

use std::sync::Arc;

use crate::app::DbNamespace;

pub async fn save_table(db_namespace: &Arc<DbNamespace>, table_name: &DbTableName) {
    match db_namespace.db.get_table(table_name.as_str()) {
        Some(db_table) => {
            let table_snapshot = db_table.get_table_snapshot();
            super::scripts::sync_table_snapshot(db_namespace, table_name, table_snapshot).await;
        }
        None => {
            super::scripts::delete_table(db_namespace, &table_name).await;
        }
    }
}
