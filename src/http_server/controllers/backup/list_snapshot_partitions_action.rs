use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

use super::inspect_models::SnapshotTableContract;

#[http_route(
    method: "GET",
    route: "/api/Backup/Partitions",
    input_data: "SnapshotTableContract",
    description: "List partitions of a table stored in a snapshot file",
    summary: "List partitions of a table stored in a snapshot file",
    controller: "Backup",
    result:[
        {status_code: 200, description: "Partitions in the snapshot table"},
        {status_code: 400, description: "Invalid file name or table not found"},
    ]
)]
pub struct ListSnapshotPartitionsAction {
    app: Arc<AppContext>,
}

impl ListSnapshotPartitionsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &ListSnapshotPartitionsAction,
    input_data: SnapshotTableContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    match crate::operations::backup::list_snapshot_partitions(
        &action.app,
        &db_namespace,
        &input_data.file_name,
        &input_data.table_name,
    )
    .await
    {
        Ok(partitions) => HttpOutput::as_json(partitions).into_ok_result(true).into(),
        Err(err) => Err(HttpFailResult::as_not_supported_content_type(
            err.into_message(),
        )),
    }
}
