use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput, WebContentType};
use std::sync::Arc;

use rust_extensions::StopWatch;

use crate::app::AppContext;

use super::contracts::GetLogsByTableName;

#[http_route(
    method: "GET",
    route: "/Logs/Table/{table_name}",
    input_data: "GetLogsByTableName"
)]
pub struct GetLogsByTableAction {
    app: Arc<AppContext>,
}

impl GetLogsByTableAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetLogsByTableAction,
    input_data: GetLogsByTableName,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let mut sw = StopWatch::new();
    sw.start();
    let logs_result = action
        .app
        .logs
        .get_by_table_name(input_data.table_name.as_str())
        .await;

    match logs_result {
        Some(logs) => super::logs::compile_result("logs by table", logs, sw).into(),
        None => {
            sw.pause();

            let content = format!(
                "Result compiled in: {:?}. No log records for the table '{}'",
                sw.duration(),
                input_data.table_name
            );

            HttpOutput::Content {
                headers: None,
                content_type: Some(WebContentType::Text),
                content: content.into_bytes(),
            }
            .into_ok_result(true)
            .into()
        }
    }
}
