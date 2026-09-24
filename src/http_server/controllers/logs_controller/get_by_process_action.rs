use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput, WebContentType};
use std::sync::Arc;

use my_no_sql_server_core::logs::*;
use rust_extensions::StopWatch;

use crate::app::AppContext;

use super::contracts::GetLogsByProcess;

#[http_route(
    method: "GET",
    route: "/Logs/Process/{process_name}",
    input_data: "GetLogsByProcess"
)]
pub struct GetLogsByProcessAction {
    app: Arc<AppContext>,
}

impl GetLogsByProcessAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetLogsByProcessAction,
    input_data: GetLogsByProcess,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let process = SystemProcess::parse(&input_data.process_name);

    if process.is_none() {
        return HttpOutput::Content {
            headers: None,
            content_type: Some(WebContentType::Text),
            content: format!("Invalid process name: {}", input_data.process_name.as_str()).into(),
        }
        .into_ok_result(true)
        .into();
    }

    let process = process.unwrap();

    let mut sw = StopWatch::new();
    sw.start();
    let logs_result = action.app.logs.get_by_process(process).await;

    match logs_result {
        Some(logs) => super::logs::compile_result("logs by process", logs, sw).into(),
        None => {
            sw.pause();

            HttpOutput::Content {
                headers: None,
                content_type: Some(WebContentType::Text),
                content: format!(
                    "Result compiled in: {:?}. No log records for the process '{}'",
                    sw.duration(),
                    input_data.process_name
                )
                .into_bytes(),
            }
            .into_ok_result(true)
        }
    }
}
