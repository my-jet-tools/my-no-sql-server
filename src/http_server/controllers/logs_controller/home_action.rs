use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult};
use rust_extensions::StopWatch;
use std::sync::Arc;

use crate::app::AppContext;

#[http_route(
    method: "GET",
    route: "/Logs",
)]
pub struct HomeAction {
    app: Arc<AppContext>,
}

impl HomeAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &HomeAction,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let mut sw = StopWatch::new();
    sw.start();
    let logs = action.app.logs.get().await;

    return super::logs::compile_result("logs", logs, sw).into();
}
