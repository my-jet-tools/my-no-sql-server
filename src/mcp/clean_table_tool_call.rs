use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use serde::*;

use crate::{
    app::AppContext,
    db_operations,
    db_sync::{DataSynchronizationPeriod, EventSource},
};

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct CleanTableInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(
        description = "Name of the table to clean (all rows in all partitions will be removed)"
    )]
    pub table_name: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct CleanTableResponse {
    #[property(description = "Outcome message")]
    pub status: String,
}

pub struct CleanTableToolCallHandler {
    app: Arc<AppContext>,
}

impl CleanTableToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for CleanTableToolCallHandler {
    const FUNC_NAME: &'static str = "clean_table";

    const DESCRIPTION: &'static str = "\
Removes ALL rows from the specified table (the table itself is kept). \
This is a destructive operation and requires MCP writes to be enabled \
by the admin in the UI Settings page (10-minute window). If this fails \
as DISABLED, ask the user to enable MCP writes — do not retry in a loop. \
See prompt 'mcp_writes_enable_policy'.";
}

#[async_trait::async_trait]
impl McpToolCall<CleanTableInputData, CleanTableResponse> for CleanTableToolCallHandler {
    async fn execute_tool_call(
        &self,
        model: CleanTableInputData,
    ) -> Result<CleanTableResponse, String> {
        let db_namespace = self
            .app
            .get_existing_namespace(model.namespace.as_deref())
            .map_err(|err| format!("{:?}", err))?;

        super::write_gate::ensure_mcp_writes_enabled(self.app.as_ref())?;

        let db_table =
            db_operations::read::table::get(self.app.as_ref(), &db_namespace, &model.table_name)
                .await
                .map_err(|err| format!("{:?}", err))?;

        let event_src = EventSource::as_client_request(self.app.as_ref());

        db_operations::write::clean_table(
            self.app.as_ref(),
            &db_namespace,
            &db_table,
            event_src,
            DataSynchronizationPeriod::Sec5.get_sync_moment(),
        )
        .await
        .map_err(|err| format!("{:?}", err))?;

        Ok(CleanTableResponse {
            status: "cleaned".into(),
        })
    }
}
