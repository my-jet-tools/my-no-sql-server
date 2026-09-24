use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

#[http_route(
    method: "POST",
    route: "/api/Persist/Force",
    deprecated_routes: ["/Persist/Force"],
    summary: "Persist everything queued",
    description: "Writes every queued change to the disk and answers when it is written",
    controller: "Persist",
    result:[
        {status_code: 204, description: "Everything queued is on the disk"},
    ]
)]
pub struct ForcePersistAction {
    app: Arc<AppContext>,
}

impl ForcePersistAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &ForcePersistAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    // Drains the queue instead of writing one task off it: the caller restarts
    // the process on this answer, and one task off a queue of fifty is data lost.
    crate::operations::persist_all(&action.app).await;
    HttpOutput::Empty.into_ok_result(true)
}
