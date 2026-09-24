use std::sync::Arc;

use crate::{
    app::{AppContext, DbNamespace},
    db_operations::DbOperationError,
};

use super::ReadOperationResult;

pub async fn start_read_all(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    table_name: &str,
) -> Result<i64, DbOperationError> {
    super::super::check_app_states(app)?;

    let db_table = super::table::get(app, db_namespace, table_name).await?;

    let entities = db_table.get_all_as_vec_dequeue();

    if entities.len() == 0 {
        return Ok(0);
    }

    let result = app.multipart_list.add(entities).await;

    Ok(result)
}

pub async fn get_next(
    app: &AppContext,
    multipart_id: i64,
    amount: usize,
) -> Option<ReadOperationResult> {
    let db_rows = app.multipart_list.get(multipart_id, amount).await?;

    ReadOperationResult::RowsArray(db_rows.as_json_array().build().into_bytes()).into()
}
