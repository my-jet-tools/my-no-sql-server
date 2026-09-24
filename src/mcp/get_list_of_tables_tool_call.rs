use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use serde::*;

use crate::app::AppContext;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetListOfTablesInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetListOfTablesResponse {
    #[property(description = "Amount of tables")]
    pub count: usize,
    #[property(description = "List of table names")]
    pub tables: Vec<String>,
}

pub struct GetListOfTablesToolCallHandler {
    app: Arc<AppContext>,
}

impl GetListOfTablesToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for GetListOfTablesToolCallHandler {
    const FUNC_NAME: &'static str = "get_list_of_tables";

    const DESCRIPTION: &'static str =
        "Returns the list of all MyNoSql table names available on this server.";
}

#[async_trait::async_trait]
impl McpToolCall<GetListOfTablesInputData, GetListOfTablesResponse>
    for GetListOfTablesToolCallHandler
{
    async fn execute_tool_call(
        &self,
        model: GetListOfTablesInputData,
    ) -> Result<GetListOfTablesResponse, String> {
        let db_namespace = self
            .app
            .get_existing_namespace(model.namespace.as_deref())
            .map_err(|err| format!("{:?}", err))?;

        let tables = db_namespace.db.get_tables();

        let tables: Vec<String> = tables.iter().map(|table| table.name.to_string()).collect();

        Ok(GetListOfTablesResponse {
            count: tables.len(),
            tables,
        })
    }
}
