use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

use super::models::TableContract;

#[http_route(
    method: "GET",
    route: "/api/Tables/List",
    deprecated_routes: ["/Tables/List"],
    description: "Get List of Tables",
    summary: "Returns List of Tables",
    controller: "Tables",
    result:[
        {status_code: 200, description: "List of tables", model: "Vec<TableContract>"},
    ]
)]
pub struct GetListAction {
    app: Arc<AppContext>,
}

impl GetListAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetListAction,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    crate::db_operations::check_app_states(action.app.as_ref())?;
    let tables = db_namespace.db.get_tables();

    let mut response: Vec<TableContract> = vec![];

    for db_table in tables.iter() {
        response.push(TableContract::from_table_wrapper(db_table));
    }

    HttpOutput::as_json(response).into_ok_result(true).into()
}
