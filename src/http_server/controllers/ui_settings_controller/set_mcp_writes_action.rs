use std::sync::Arc;

use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};

use crate::app::AppContext;

use super::models::{McpWritesBody, McpWritesInput, SettingsPublicModel};

#[http_route(
    method: "POST",
    route: "/api/Settings/McpWrites",
    controller: "Settings",
    description: "Enables or disables MCP write tools (delete_row, bulk_delete_rows, insert_or_replace_row, clean_table, delete_partitions). Enabling opens a 10-minute window after which writes auto-disable; disabling closes it immediately. Runtime-only — a server restart leaves MCP writes disabled.",
    summary: "Enable/disable MCP writes",
    input_data: McpWritesInput,
    result:[
        {status_code: 200, description: "Updated settings", model: "SettingsPublicModel"},
    ]
)]
pub struct SetMcpWritesAction {
    app: Arc<AppContext>,
}

impl SetMcpWritesAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &SetMcpWritesAction,
    input_data: McpWritesInput,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let body: McpWritesBody = match serde_json::from_slice(input_data.body.as_slice()) {
        Ok(b) => b,
        Err(err) => {
            return Err(HttpFailResult::as_validation_error(format!(
                "Invalid body: {}",
                err
            )));
        }
    };

    if body.enabled {
        action.app.enable_mcp_writes();
    } else {
        action.app.disable_mcp_writes();
    }

    let settings = super::storage::load(action.app.settings.persistence_dest.as_str()).await;
    HttpOutput::as_json(SettingsPublicModel::new(&settings, action.app.as_ref()))
        .into_ok_result(false)
        .into()
}
