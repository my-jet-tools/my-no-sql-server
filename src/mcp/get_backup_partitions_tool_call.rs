use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use serde::*;

use crate::app::AppContext;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetBackupPartitionsInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(description = "Snapshot file name (as returned by get_list_of_backups)")]
    pub file_name: String,
    #[property(description = "Table name inside the snapshot (as returned by get_backup_tables)")]
    pub table_name: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetBackupPartitionsResponse {
    #[property(description = "Amount of partitions")]
    pub count: usize,
    #[property(description = "List of partition keys stored for the table in the snapshot")]
    pub partitions: Vec<String>,
}

pub struct GetBackupPartitionsToolCallHandler {
    app: Arc<AppContext>,
}

impl GetBackupPartitionsToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for GetBackupPartitionsToolCallHandler {
    const FUNC_NAME: &'static str = "get_backup_partitions";

    const DESCRIPTION: &'static str =
        "Returns the list of partition keys stored for a table inside a snapshot (backup) file. Use get_backup_tables first to find the table_name.";
}

#[async_trait::async_trait]
impl McpToolCall<GetBackupPartitionsInputData, GetBackupPartitionsResponse>
    for GetBackupPartitionsToolCallHandler
{
    async fn execute_tool_call(
        &self,
        model: GetBackupPartitionsInputData,
    ) -> Result<GetBackupPartitionsResponse, String> {
        let db_namespace = self
            .app
            .get_existing_namespace(model.namespace.as_deref())
            .map_err(|err| format!("{:?}", err))?;

        let partitions = crate::operations::backup::list_snapshot_partitions(
            self.app.as_ref(),
            &db_namespace,
            &model.file_name,
            &model.table_name,
        )
        .await
        .map_err(|err| err.into_message())?;

        Ok(GetBackupPartitionsResponse {
            count: partitions.len(),
            partitions,
        })
    }
}
