use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput, WebContentType};
use rust_extensions::StopWatch;
use std::sync::Arc;

use crate::app::AppContext;

#[http_route(
    method: "GET",
    route: "/Logs/FatalErrors",
)]
pub struct GetFatalErrorsAction {
    app: Arc<AppContext>,
}

impl GetFatalErrorsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetFatalErrorsAction,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let mut sw = StopWatch::new();
    sw.start();
    let logs_result = action.app.logs.get_fatal_errors().await;

    match logs_result {
        Some(logs) => super::logs::compile_result("FatalError logs", logs, sw).into(),
        None => {
            sw.pause();

            let content = format!(
                "Result compiled in: {:?}. No fatal error records",
                sw.duration(),
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
