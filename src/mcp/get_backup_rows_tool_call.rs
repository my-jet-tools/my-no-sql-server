use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use serde::*;

use crate::app::AppContext;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetBackupRowsInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(description = "Snapshot file name (as returned by get_list_of_backups)")]
    pub file_name: String,
    #[property(description = "Table name inside the snapshot (as returned by get_backup_tables)")]
    pub table_name: String,
    #[property(
        description = "Partition key inside the table (as returned by get_backup_partitions)"
    )]
    pub partition_key: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetBackupRowsResponse {
    #[property(description = "Amount of rows returned")]
    pub count: usize,
    #[property(description = "List of rows. Each item is a JSON object encoded as a string")]
    pub rows: Vec<String>,
}

pub struct GetBackupRowsToolCallHandler {
    app: Arc<AppContext>,
}

impl GetBackupRowsToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for GetBackupRowsToolCallHandler {
    const FUNC_NAME: &'static str = "get_backup_rows";

    const DESCRIPTION: &'static str =
        "Returns the rows of a partition stored inside a snapshot (backup) file. Use get_backup_partitions first to find the partition_key.";
}

#[async_trait::async_trait]
impl McpToolCall<GetBackupRowsInputData, GetBackupRowsResponse> for GetBackupRowsToolCallHandler {
    async fn execute_tool_call(
        &self,
        model: GetBackupRowsInputData,
    ) -> Result<GetBackupRowsResponse, String> {
        let db_namespace = self
            .app
            .get_existing_namespace(model.namespace.as_deref())
            .map_err(|err| format!("{:?}", err))?;

        let content = crate::operations::backup::read_snapshot_partition_rows(
            self.app.as_ref(),
            &db_namespace,
            &model.file_name,
            &model.table_name,
            &model.partition_key,
        )
        .await
        .map_err(|err| err.into_message())?;

        // The partition content is stored as a JSON array of row objects.
        let rows: Vec<String> = match serde_json::from_slice::<Vec<serde_json::Value>>(&content) {
            Ok(values) => values.iter().map(|value| value.to_string()).collect(),
            Err(_) => vec![String::from_utf8_lossy(&content).into_owned()],
        };

        Ok(GetBackupRowsResponse {
            count: rows.len(),
            rows,
        })
    }
}
