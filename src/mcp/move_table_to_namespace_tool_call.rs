use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use serde::*;

use crate::{app::AppContext, db_operations, db_sync::EventSource};

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct MoveTableToNamespaceInputData {
    #[property(description = "Name of the table to move")]
    pub table_name: String,
    #[property(
        description = "Namespace the table is in right now. Empty means the default namespace"
    )]
    pub from_namespace: Option<String>,
    #[property(
        description = "Namespace to move the table into. Empty means the default namespace. Created if it does not exist yet"
    )]
    pub to_namespace: Option<String>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct MoveTableToNamespaceResponse {
    #[property(description = "Outcome message")]
    pub status: String,
    #[property(description = "Namespace the table was taken from")]
    pub from_namespace: String,
    #[property(description = "Namespace the table now lives in")]
    pub to_namespace: String,
}

pub struct MoveTableToNamespaceToolCallHandler {
    app: Arc<AppContext>,
}

impl MoveTableToNamespaceToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for MoveTableToNamespaceToolCallHandler {
    const FUNC_NAME: &'static str = "move_table_to_namespace";

    const DESCRIPTION: &'static str = "\
Moves a whole table, with all of its rows, from one namespace into another. \
The table stops existing in the source namespace: readers subscribed to it there \
are told it is gone, and readers of the destination namespace get it initialized. \
Refused when the destination namespace already has a table of that name — move or \
delete that one first, nothing is overwritten. \
This is a destructive operation and requires MCP writes to be enabled by the admin \
in the UI Settings page (10-minute window). If this fails as DISABLED, ask the user \
to enable MCP writes — do not retry in a loop. See prompt 'mcp_writes_enable_policy'.";
}

#[async_trait::async_trait]
impl McpToolCall<MoveTableToNamespaceInputData, MoveTableToNamespaceResponse>
    for MoveTableToNamespaceToolCallHandler
{
    async fn execute_tool_call(
        &self,
        model: MoveTableToNamespaceInputData,
    ) -> Result<MoveTableToNamespaceResponse, String> {
        super::write_gate::ensure_mcp_writes_enabled(self.app.as_ref())?;

        // The source has to exist — there is nothing to move out of a namespace
        // nobody ever wrote to. The destination may well be new: moving a table
        // into a fresh namespace is the point of the tool.
        let from = self
            .app
            .get_existing_namespace(model.from_namespace.as_deref())
            .map_err(|err| format!("{:?}", err))?;

        let to = self
            .app
            .get_or_create_namespace(model.to_namespace.as_deref())
            .await
            .map_err(|err| format!("{:?}", err))?;

        let event_src = EventSource::as_client_request(self.app.as_ref());

        db_operations::write::move_table_to_namespace(
            &self.app,
            &from,
            &to,
            model.table_name.as_str(),
            event_src,
        )
        .await
        .map_err(|err| format!("{:?}", err))?;

        Ok(MoveTableToNamespaceResponse {
            status: format!(
                "Table '{}' moved from '{}' to '{}'",
                model.table_name, from.name, to.name
            ),
            from_namespace: from.name.to_string(),
            to_namespace: to.name.to_string(),
        })
    }
}
