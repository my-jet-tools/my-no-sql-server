use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::{app::AppContext, db_sync::EventSource};

use super::models::CleanAndKeepMaxPartitionsAmountInputContract;

#[http_route(
    method: "POST",
    route: "/api/GarbageCollector/CleanAndKeepMaxPartitions",
    deprecated_routes: ["/GarbageCollector/CleanAndKeepMaxPartitions"],
    summary: "Makes sure we keep maximum partitions amount required",
    description: "After operation some partitions can be deleted to make sure we keep maximum partitions amount required",
    controller: "GarbageCollector",
    input_data: CleanAndKeepMaxPartitionsAmountInputContract,
    result:[
        {status_code: 202, description: "Successful operation"},
        {status_code: 400, description: "Table not found"}
    ]
)]
pub struct CleanAndKeepMaxPartitionsAmountAction {
    app: Arc<AppContext>,
}

impl CleanAndKeepMaxPartitionsAmountAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &CleanAndKeepMaxPartitionsAmountAction,
    http_input: CleanAndKeepMaxPartitionsAmountInputContract,
    ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        http_input.table_name.as_str(),
    )
    .await?;

    let event_src = EventSource::as_client_request(action.app.as_ref());

    crate::db_operations::gc::keep_max_partitions_amount(
        action.app.as_ref(),
        &db_namespace,
        &db_table,
        http_input.max_partitions_amount,
        event_src,
        http_input.sync_period.get_sync_moment(),
    )
    .await?;

    HttpOutput::Empty.into_ok_result(true).into()
}
