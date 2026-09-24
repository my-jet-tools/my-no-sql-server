use std::sync::Arc;

use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};

use crate::app::AppContext;

use super::models::{SettingsPublicModel, UiWritesBody, UiWritesInput};

#[http_route(
    method: "POST",
    route: "/api/Settings/UiWrites",
    controller: "Settings",
    description: "Enables or disables destructive write operations in the UI (delete row, bulk delete, paste & delete, restore from backup). Enabling opens a 10-minute window after which writes auto-disable; disabling closes it immediately. Runtime-only — a server restart leaves UI writes disabled. Independent of the MCP-writes window.",
    summary: "Enable/disable UI writes",
    input_data: UiWritesInput,
    result:[
        {status_code: 200, description: "Updated settings", model: "SettingsPublicModel"},
    ]
)]
pub struct SetUiWritesAction {
    app: Arc<AppContext>,
}

impl SetUiWritesAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &SetUiWritesAction,
    input_data: UiWritesInput,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let body: UiWritesBody = match serde_json::from_slice(input_data.body.as_slice()) {
        Ok(b) => b,
        Err(err) => {
            return Err(HttpFailResult::as_validation_error(format!(
                "Invalid body: {}",
                err
            )));
        }
    };

    if body.enabled {
        action.app.enable_ui_writes();
    } else {
        action.app.disable_ui_writes();
    }

    let settings = super::storage::load(action.app.settings.persistence_dest.as_str()).await;
    HttpOutput::as_json(SettingsPublicModel::new(&settings, action.app.as_ref()))
        .into_ok_result(false)
        .into()
}
