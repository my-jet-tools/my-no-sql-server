use std::sync::Arc;

use my_no_sql_sdk::server::DbTable;

use crate::{
    app::{AppContext, DbNamespace},
    db_operations::DbOperationError,
};

pub async fn get(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    table_name: &str,
) -> Result<Arc<DbTable>, DbOperationError> {
    super::super::check_app_states(app)?;

    let get_table_result = db_namespace.db.get_table(table_name);

    match get_table_result {
        Some(db_table) => Ok(db_table),
        None => Err(DbOperationError::TableNotFound(table_name.to_string())),
    }
}
