use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use my_no_sql_sdk::core::db_json_entity::JsonTimeStamp;
use serde::*;

use crate::{app::AppContext, db_operations::UpdateStatistics};

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetRowsInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(description = "Name of the table to query")]
    pub table_name: String,
    #[property(description = "Optional partition key filter")]
    pub partition_key: Option<String>,
    #[property(description = "Optional row key filter")]
    pub row_key: Option<String>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetRowsResponse {
    #[property(description = "Amount of rows returned")]
    pub count: usize,
    #[property(description = "List of rows. Each item is a JSON object encoded as a string")]
    pub rows: Vec<String>,
}

pub struct GetRowsToolCallHandler {
    app: Arc<AppContext>,
}

impl GetRowsToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for GetRowsToolCallHandler {
    const FUNC_NAME: &'static str = "get_rows";

    const DESCRIPTION: &'static str =
        "Returns rows from a MyNoSql table. Filter by partition_key and/or row_key (both optional).";
}

#[async_trait::async_trait]
impl McpToolCall<GetRowsInputData, GetRowsResponse> for GetRowsToolCallHandler {
    async fn execute_tool_call(&self, model: GetRowsInputData) -> Result<GetRowsResponse, String> {
        let db_namespace = self
            .app
            .get_existing_namespace(model.namespace.as_deref())
            .map_err(|err| format!("{:?}", err))?;

        let table = crate::db_operations::read::table::get(
            self.app.as_ref(),
            &db_namespace,
            &model.table_name,
        )
        .await
        .map_err(|err| format!("{:?}", err))?;

        let now = JsonTimeStamp::now();

        let update_statistics = UpdateStatistics {
            update_partition_last_read_access_time: false,
            update_rows_last_read_access_time: false,
            update_partition_expiration_time: None,
            update_rows_expiration_time: None,
        };

        let db_rows = crate::db_operations::read::get_rows_as_vec::execute(
            &self.app,
            &table,
            model.partition_key.as_ref(),
            model.row_key.as_ref(),
            None,
            None,
            &now,
            update_statistics,
        )
        .await
        .map_err(|err| format!("{:?}", err))?;

        let rows: Vec<String> = db_rows
            .into_iter()
            .map(|db_row| String::from_utf8_lossy(&db_row.to_vec()).into_owned())
            .collect();

        Ok(GetRowsResponse {
            count: rows.len(),
            rows,
        })
    }
}
