use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use serde::*;

use crate::{
    app::AppContext,
    db_operations,
    db_sync::{DataSynchronizationPeriod, EventSource},
};

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct DeleteRowInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(description = "Name of the table")]
    pub table_name: String,
    #[property(description = "Partition key")]
    pub partition_key: String,
    #[property(description = "Row key")]
    pub row_key: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct DeleteRowResponse {
    #[property(description = "Outcome message")]
    pub status: String,
}

pub struct DeleteRowToolCallHandler {
    app: Arc<AppContext>,
}

impl DeleteRowToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for DeleteRowToolCallHandler {
    const FUNC_NAME: &'static str = "delete_row";

    const DESCRIPTION: &'static str = "\
Deletes a single row by partition_key + row_key. Requires MCP writes to \
be enabled by the admin in the UI Settings page (10-minute window). If \
this fails as DISABLED, ask the user to enable MCP writes — do not retry \
in a loop. See prompt 'mcp_writes_enable_policy'.";
}

#[async_trait::async_trait]
impl McpToolCall<DeleteRowInputData, DeleteRowResponse> for DeleteRowToolCallHandler {
    async fn execute_tool_call(
        &self,
        model: DeleteRowInputData,
    ) -> Result<DeleteRowResponse, String> {
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
        let now = DateTimeAsMicroseconds::now();

        db_operations::write::delete_row::execute(
            self.app.as_ref(),
            &db_namespace,
            &db_table,
            model.partition_key,
            model.row_key,
            event_src,
            DataSynchronizationPeriod::Sec5.get_sync_moment(),
            now,
        )
        .await
        .map_err(|err| format!("{:?}", err))?;

        Ok(DeleteRowResponse {
            status: "deleted".into(),
        })
    }
}
