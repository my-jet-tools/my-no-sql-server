use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput, WebContentType};
use std::sync::Arc;

use crate::app::AppContext;

use super::inspect_models::SnapshotPartitionContract;

#[http_route(
    method: "GET",
    route: "/api/Backup/Rows",
    input_data: "SnapshotPartitionContract",
    description: "Get rows of a partition stored in a snapshot file",
    summary: "Get rows of a partition stored in a snapshot file",
    controller: "Backup",
    result:[
        {status_code: 200, description: "Rows in the snapshot partition (JSON array)"},
        {status_code: 400, description: "Invalid file name or partition not found"},
    ]
)]
pub struct GetSnapshotRowsAction {
    app: Arc<AppContext>,
}

impl GetSnapshotRowsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetSnapshotRowsAction,
    input_data: SnapshotPartitionContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    match crate::operations::backup::read_snapshot_partition_rows(
        &action.app,
        &db_namespace,
        &input_data.file_name,
        &input_data.table_name,
        &input_data.partition_key,
    )
    .await
    {
        Ok(content) => HttpOutput::Content {
            headers: WebContentType::Json.into(),
            status_code: 200,
            content,
        }
        .into_ok_result(true)
        .into(),
        Err(err) => Err(HttpFailResult::as_not_supported_content_type(
            err.into_message(),
        )),
    }
}
