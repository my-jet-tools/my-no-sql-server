use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use serde::*;

use crate::app::AppContext;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetBackupTablesInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(description = "Snapshot file name (as returned by get_list_of_backups)")]
    pub file_name: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct BackupTableModel {
    #[property(description = "Table name")]
    pub name: String,
    #[property(description = "Amount of partitions stored for this table in the snapshot")]
    pub partitions_count: usize,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetBackupTablesResponse {
    #[property(description = "Amount of tables in the snapshot")]
    pub count: usize,
    #[property(description = "List of tables stored in the snapshot file")]
    pub tables: Vec<BackupTableModel>,
}

pub struct GetBackupTablesToolCallHandler {
    app: Arc<AppContext>,
}

impl GetBackupTablesToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for GetBackupTablesToolCallHandler {
    const FUNC_NAME: &'static str = "get_backup_tables";

    const DESCRIPTION: &'static str =
        "Returns the list of tables stored inside a snapshot (backup) file. Use get_list_of_backups first to find the file_name.";
}

#[async_trait::async_trait]
impl McpToolCall<GetBackupTablesInputData, GetBackupTablesResponse>
    for GetBackupTablesToolCallHandler
{
    async fn execute_tool_call(
        &self,
        model: GetBackupTablesInputData,
    ) -> Result<GetBackupTablesResponse, String> {
        let db_namespace = self
            .app
            .get_existing_namespace(model.namespace.as_deref())
            .map_err(|err| format!("{:?}", err))?;

        let tables = crate::operations::backup::list_snapshot_tables(
            self.app.as_ref(),
            &db_namespace,
            &model.file_name,
        )
        .await
        .map_err(|err| err.into_message())?;

        let tables: Vec<BackupTableModel> = tables
            .into_iter()
            .map(|table| BackupTableModel {
                name: table.name,
                partitions_count: table.partitions_count,
            })
            .collect();

        Ok(GetBackupTablesResponse {
            count: tables.len(),
            tables,
        })
    }
}
