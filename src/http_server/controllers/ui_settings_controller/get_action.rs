use std::sync::Arc;

use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};

use crate::app::AppContext;

use super::models::SettingsPublicModel;

#[http_route(
    method: "GET",
    route: "/api/Settings",
    controller: "Settings",
    description: "Returns server settings: UI health thresholds plus the runtime MCP-writes enable state (whether writes are currently enabled and how many seconds remain in the window).",
    summary: "Read settings",
    result:[
        {status_code: 200, description: "Settings", model: "SettingsPublicModel"},
    ]
)]
pub struct GetUiSettingsAction {
    app: Arc<AppContext>,
}

impl GetUiSettingsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetUiSettingsAction,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let model = super::storage::load(action.app.settings.persistence_dest.as_str()).await;
    let public = SettingsPublicModel::new(&model, action.app.as_ref());
    HttpOutput::as_json(public).into_ok_result(false).into()
}
