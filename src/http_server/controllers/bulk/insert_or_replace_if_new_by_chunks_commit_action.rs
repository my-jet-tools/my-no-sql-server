use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use std::sync::Arc;

use crate::app::AppContext;
use crate::db_sync::EventSource;

use super::models::BulkProcessInputContract;

#[http_route(
    method: "POST",
    route: "/api/Bulk/InsertOrReplaceIfNewByChunksCommit",
    input_data: "BulkProcessInputContract",
    summary: "Commits a chunked insert-or-replace-if-new operation",
    description: "Applies all the accumulated rows in a single operation: each row is inserted when missing, or replaces the stored one only when its TimeStamp is greater. The table is not cleaned",
    controller: "Bulk",
    result:[
        {status_code: 202, description: "Successful operation"},
        {status_code: 400, description: "Table not found"},
        {status_code: 404, description: "Process not found - it expired, was committed or the server was restarted"},
        {status_code: 409, description: "Process belongs to another writer session"},
    ]
)]
pub struct InsertOrReplaceIfNewByChunksCommitAction {
    app: Arc<AppContext>,
}

impl InsertOrReplaceIfNewByChunksCommitAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &InsertOrReplaceIfNewByChunksCommitAction,
    input_data: BulkProcessInputContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace(&action.app, ctx).await?;

    let event_src = EventSource::as_client_request(action.app.as_ref());

    let now = DateTimeAsMicroseconds::now();

    let session = input_data.session_id.as_deref().filter(|s| !s.is_empty());

    crate::db_operations::bulk_processes::commit(
        action.app.as_ref(),
        &db_namespace,
        input_data.process_id.as_str(),
        session,
        event_src,
        input_data.sync_period.get_sync_moment(),
        now,
    )
    .await?;

    HttpOutput::Empty.into_ok_result(true).into()
}
