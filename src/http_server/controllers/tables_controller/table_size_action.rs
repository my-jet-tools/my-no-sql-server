use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

use super::models::GetTableSizeContract;

#[http_route(
    method: "GET",
    route: "/api/Tables/TableSize",
    deprecated_routes: ["/Tables/TableSize"],
    input_data: "GetTableSizeContract",
    description: "Get Table size",
    summary: "Returns Table size",
    controller: "Tables",
    result:[
        {status_code: 200, description: "Size of table", model: "Long"},
        {status_code: 400, description: "Table not found"},
    ]
)]
pub struct GetTableSizeAction {
    app: Arc<AppContext>,
}

impl GetTableSizeAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetTableSizeAction,
    input_data: GetTableSizeContract,
    ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    crate::db_operations::check_app_states(action.app.as_ref())?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_str(),
    )
    .await?;

    let partitions_amount = db_table.get_table_size();

    HttpOutput::as_text(format!("{}", partitions_amount))
        .into_ok_result(true)
        .into()
}
