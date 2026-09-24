use super::models::*;
use crate::app::AppContext;
use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::{result::Result, sync::Arc};

#[http_route(
    method: "GET",
    route: "/api/Partitions",
    deprecated_routes: ["/Partitions"],
    input_data: "GetPartitionsListContract",
    description: "Get Partitions amount of selected table",
    summary: "Returns Partitions amount of selected table",
    controller: "Partitions",
    result:[
        {status_code: 200, description: "Partitions amount", model: "PartitionsHttpResult"},
        {status_code: 400, description: "Table not found"},
    ]
)]
pub struct GetPartitionsAction {
    app: Arc<AppContext>,
}

impl GetPartitionsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetPartitionsAction,
    input_data: GetPartitionsListContract,
    ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_str(),
    )
    .await?;

    let result = crate::db_operations::read::partitions::get_partitions(
        &action.app,
        &db_table,
        input_data.limit,
        input_data.skip,
    )
    .await?;

    let result = PartitionsHttpResult {
        amount: result.0,
        data: result.1,
    };

    HttpOutput::as_json(result).into_ok_result(true).into()
}
