use my_no_sql_sdk::core::db::DbTableName;

use std::sync::Arc;

use crate::app::DbNamespace;

pub async fn save_table_attributes(db_namespace: &Arc<DbNamespace>, table_name: &DbTableName) {
    match db_namespace.db.get_table(table_name.as_str()) {
        Some(db_table) => {
            let attr = db_table.get_attributes();
            db_namespace
                .repo
                .save_table_metadata(table_name, &attr)
                .await;
        }
        None => {
            super::scripts::delete_table(db_namespace, &table_name).await;
        }
    }
}
