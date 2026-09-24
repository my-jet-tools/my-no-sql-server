use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

#[http_route(
    method: "POST",
    route: "/api/Backup/RestorePartition",
    description: "Restore a single partition of a table from a backup file",
    summary: "Restore a single partition of a table from a backup file",
    controller: "Backup",
    input_data: RestorePartitionInputData,
    result:[
        {status_code: 204, description: "Restored ok"},
    ]
)]
pub struct RestorePartitionFromBackupAction {
    app: Arc<AppContext>,
}

impl RestorePartitionFromBackupAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &RestorePartitionFromBackupAction,
    input_data: RestorePartitionInputData,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace(&action.app, ctx).await?;

    let restore_result = crate::operations::backup::restore_partition(
        &action.app,
        &db_namespace,
        &input_data.file_name,
        &input_data.table_name,
        &input_data.partition_key,
    )
    .await;

    match restore_result {
        Ok(_) => HttpOutput::Empty.into_ok_result(true).into(),
        Err(err) => Err(HttpFailResult::as_fatal_error(err)),
    }
}

#[derive(MyHttpInput)]
pub struct RestorePartitionInputData {
    #[http_form_data(name = "fileName", description = "File in backup folder")]
    pub file_name: String,

    #[http_form_data(name = "tableName", description = "Name of the table")]
    pub table_name: String,

    #[http_form_data(name = "partitionKey", description = "Partition key to restore")]
    pub partition_key: String,
}
