use super::models::GetPartitionsAmountContract;
use crate::app::AppContext;
use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::{result::Result, sync::Arc};

#[http_route(
    method: "GET",
    route: "/api/Partitions/Count",
    deprecated_routes: ["/Partitions/Count"],
    input_data: "GetPartitionsAmountContract",
    description: "Get Partitions amount of selected table",
    summary: "Returns Partitions amount of selected table",
    controller: "Partitions",
    result:[
        {status_code: 200, description: "Partitions amount", model: "Long"},
        {status_code: 400, description: "Table not found"},
    ]
)]
pub struct GetPartitionsCountAction {
    app: Arc<AppContext>,
}

impl GetPartitionsCountAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetPartitionsCountAction,
    input_data: GetPartitionsAmountContract,
    ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_str(),
    )
    .await?;

    let partitions_amount = db_table.get_partitions_amount();

    HttpOutput::as_text(format!("{}", partitions_amount))
        .into_ok_result(true)
        .into()
}
