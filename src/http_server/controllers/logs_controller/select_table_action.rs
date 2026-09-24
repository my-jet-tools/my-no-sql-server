use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult};
use std::sync::Arc;

use crate::app::AppContext;

#[http_route(
    method: "GET",
    route: "/Logs/Table",
)]
pub struct SelectTableAction {
    app: Arc<AppContext>,
}

impl SelectTableAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &SelectTableAction,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let mut body = String::new();

    body.push_str("<h1>Please, select table to show logs</h1>");

    for table_name in action.app.db.get_table_names().iter() {
        let line = format!(
            "<a class='btn btn-sm btn-outline-primary' href='/logs/table/{table_name}'>{table_name}</a>",
        );
        body.push_str(line.as_str());
    }

    super::super::as_html::build("Select table to show logs", body.as_str()).into_ok_result(true)
}
