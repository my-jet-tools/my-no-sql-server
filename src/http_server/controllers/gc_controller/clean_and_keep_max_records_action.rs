use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::{app::AppContext, db_sync::EventSource};

use super::models::CleanPartitionAndKeepMaxRowsAmountInputContract;

#[http_route(
    method: "POST",
    route: "/api/GarbageCollector/CleanAndKeepMaxRecords",
    deprecated_routes: ["/GarbageCollector/CleanAndKeepMaxRecords"],
    summary: "Makes sure we keep maximum rows amount required",
    description: "After operation some rows are going to be deleted to make sure we keep maximum rows amount required",
    controller: "GarbageCollector",
    input_data: "CleanPartitionAndKeepMaxRowsAmountInputContract",
    result:[
        {status_code: 202, description: "Successful operation"},
        {status_code: 400, description: "Table not found"}
    ]
)]
pub struct CleanPartitionAndKepMaxRecordsControllerAction {
    app: Arc<AppContext>,
}

impl CleanPartitionAndKepMaxRecordsControllerAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request<'s>(
    action: &CleanPartitionAndKepMaxRecordsControllerAction,
    input_data: CleanPartitionAndKeepMaxRowsAmountInputContract,
    ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_ref(),
    )
    .await?;

    let event_src = EventSource::as_client_request(action.app.as_ref());

    crate::db_operations::gc::keep_partition_max_records::execute(
        action.app.as_ref(),
        &db_namespace,
        db_table.as_ref(),
        input_data.partition_key,
        input_data.max_amount,
        event_src,
        input_data.sync_period.get_sync_moment(),
    )
    .await?;

    HttpOutput::Empty.into_ok_result(true).into()
}
