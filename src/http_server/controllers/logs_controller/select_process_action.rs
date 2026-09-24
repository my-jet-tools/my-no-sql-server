use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult};
use my_no_sql_server_core::logs::*;

#[http_route(
    method: "GET",
    route: "/Logs/Process",

)]
pub struct SelectProcessAction {}

impl SelectProcessAction {
    pub fn new() -> Self {
        Self {}
    }
}

async fn handle_request(
    _: &SelectProcessAction,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let mut sb = String::new();

    sb.push_str("<h1>Please, select process to show logs</h1>");

    for process in &SystemProcess::iterate() {
        let line = format!(
            "<a class='btn btn-sm btn-outline-primary' href='/logs/process/{process:?}'>{process:?}</a>",
            process = process
        );
        sb.push_str(line.as_str())
    }

    super::super::as_html::build("Select table to show logs", sb.as_str()).into_ok_result(true)
}
