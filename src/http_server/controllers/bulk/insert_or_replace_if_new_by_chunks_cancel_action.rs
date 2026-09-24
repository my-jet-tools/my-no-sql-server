use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

use super::models::CancelBulkProcessInputContract;

#[http_route(
    method: "POST",
    route: "/api/Bulk/InsertOrReplaceIfNewByChunksCancel",
    input_data: "CancelBulkProcessInputContract",
    summary: "Cancels a chunked insert-or-replace-if-new operation",
    description: "Drops the accumulated rows. The table is not touched",
    controller: "Bulk",
    result:[
        {status_code: 202, description: "Process is canceled"},
        {status_code: 404, description: "Process not found"},
        {status_code: 409, description: "Process belongs to another writer session"},
    ]
)]
pub struct InsertOrReplaceIfNewByChunksCancelAction {
    app: Arc<AppContext>,
}

impl InsertOrReplaceIfNewByChunksCancelAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &InsertOrReplaceIfNewByChunksCancelAction,
    input_data: CancelBulkProcessInputContract,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let session = input_data.session_id.as_deref().filter(|s| !s.is_empty());

    action
        .app
        .active_bulk_processes
        .cancel(input_data.process_id.as_str(), session)
        .await?;

    HttpOutput::Empty.into_ok_result(true).into()
}
