use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use my_no_sql_sdk::core::db_json_entity::JsonTimeStamp;
use serde::*;

use crate::{
    app::AppContext,
    db_operations,
    db_sync::{DataSynchronizationPeriod, EventSource},
};

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct InsertOrReplaceRowInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(description = "Name of the table")]
    pub table_name: String,
    #[property(
        description = "Full row JSON. Must include 'PartitionKey' and 'RowKey' string fields plus any additional row data."
    )]
    pub entity_json: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct InsertOrReplaceRowResponse {
    #[property(description = "Outcome message")]
    pub status: String,
}

pub struct InsertOrReplaceRowToolCallHandler {
    app: Arc<AppContext>,
}

impl InsertOrReplaceRowToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for InsertOrReplaceRowToolCallHandler {
    const FUNC_NAME: &'static str = "insert_or_replace_row";

    const DESCRIPTION: &'static str = "\
Inserts a row or replaces it if a row with the same PartitionKey + \
RowKey already exists. Requires MCP writes to be enabled by the admin in \
the UI Settings page (10-minute window). If this fails as DISABLED, ask \
the user to enable MCP writes — do not retry in a loop. See prompt \
'mcp_writes_enable_policy'.";
}

#[async_trait::async_trait]
impl McpToolCall<InsertOrReplaceRowInputData, InsertOrReplaceRowResponse>
    for InsertOrReplaceRowToolCallHandler
{
    async fn execute_tool_call(
        &self,
        model: InsertOrReplaceRowInputData,
    ) -> Result<InsertOrReplaceRowResponse, String> {
        let db_namespace = self
            .app
            .get_or_create_namespace(model.namespace.as_deref())
            .await
            .map_err(|err| format!("{:?}", err))?;

        super::write_gate::ensure_mcp_writes_enabled(self.app.as_ref())?;

        let db_table =
            db_operations::read::table::get(self.app.as_ref(), &db_namespace, &model.table_name)
                .await
                .map_err(|err| format!("{:?}", err))?;

        let event_src = EventSource::as_client_request(self.app.as_ref());
        let now = JsonTimeStamp::now();

        let db_row = crate::operations::parse_db_json_entity(model.entity_json.as_bytes(), &now)
            .map_err(|err| format!("{:?}", err))?;

        db_operations::write::insert_or_replace::execute(
            self.app.as_ref(),
            &db_namespace,
            db_table,
            Arc::new(db_row),
            event_src,
            DataSynchronizationPeriod::Sec5.get_sync_moment(),
            now.date_time,
        )
        .await
        .map_err(|err| format!("{:?}", err))?;

        Ok(InsertOrReplaceRowResponse {
            status: "ok".into(),
        })
    }
}
