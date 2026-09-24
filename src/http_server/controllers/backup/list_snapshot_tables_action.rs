use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

use super::inspect_models::SnapshotFileContract;

#[http_route(
    method: "GET",
    route: "/api/Backup/Tables",
    input_data: "SnapshotFileContract",
    description: "List tables stored in a snapshot file",
    summary: "List tables stored in a snapshot file",
    controller: "Backup",
    result:[
        {status_code: 200, description: "Tables in the snapshot"},
        {status_code: 400, description: "Invalid file name or snapshot not found"},
    ]
)]
pub struct ListSnapshotTablesAction {
    app: Arc<AppContext>,
}

impl ListSnapshotTablesAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &ListSnapshotTablesAction,
    input_data: SnapshotFileContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    match crate::operations::backup::list_snapshot_tables(
        &action.app,
        &db_namespace,
        &input_data.file_name,
    )
    .await
    {
        Ok(tables) => HttpOutput::as_json(tables).into_ok_result(true).into(),
        Err(err) => Err(HttpFailResult::as_not_supported_content_type(
            err.into_message(),
        )),
    }
}
