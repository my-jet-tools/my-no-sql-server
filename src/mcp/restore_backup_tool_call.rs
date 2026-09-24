use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use serde::*;

use crate::app::AppContext;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct RestoreBackupInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(description = "Snapshot file name (as returned by get_list_of_backups)")]
    pub file_name: String,

    #[property(
        description = "Table to restore. Omit (or pass '*') to restore ALL tables found in the snapshot; pass a table name to restore just that one."
    )]
    pub table_name: Option<String>,

    #[property(
        description = "When true, existing rows of each restored table are deleted before the snapshot rows are loaded. Defaults to false."
    )]
    pub clean_table: Option<bool>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct RestoreBackupResponse {
    #[property(description = "Outcome message")]
    pub status: String,
}

pub struct RestoreBackupToolCallHandler {
    app: Arc<AppContext>,
}

impl RestoreBackupToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for RestoreBackupToolCallHandler {
    const FUNC_NAME: &'static str = "restore_backup";

    const DESCRIPTION: &'static str =
        "Restores tables from a snapshot (backup) file in the server's backup folder. Omit table_name (or pass '*') to restore ALL tables found in the snapshot; pass a single table name to restore just that one. This is a destructive write operation and requires MCP writes to be enabled. Use get_list_of_backups to find the file_name and get_backup_tables to inspect its contents first.";
}

#[async_trait::async_trait]
impl McpToolCall<RestoreBackupInputData, RestoreBackupResponse> for RestoreBackupToolCallHandler {
    async fn execute_tool_call(
        &self,
        model: RestoreBackupInputData,
    ) -> Result<RestoreBackupResponse, String> {
        let db_namespace = self
            .app
            .get_or_create_namespace(model.namespace.as_deref())
            .await
            .map_err(|err| format!("{:?}", err))?;

        super::write_gate::ensure_mcp_writes_enabled(self.app.as_ref())?;

        // Empty / "*" means "restore every table in the snapshot".
        let table_name = match model.table_name.as_deref() {
            None | Some("") | Some("*") => None,
            Some(table) => Some(table),
        };
        let clean_table = model.clean_table.unwrap_or(false);

        crate::operations::backup::restore_from_file(
            &self.app,
            &db_namespace,
            &model.file_name,
            table_name,
            clean_table,
        )
        .await
        .map_err(|err| err.into_message())?;

        let status = match table_name {
            Some(table) => format!(
                "Table '{}' has been restored from snapshot '{}'.",
                table, model.file_name
            ),
            None => format!(
                "All tables have been restored from snapshot '{}'.",
                model.file_name
            ),
        };

        Ok(RestoreBackupResponse { status })
    }
}
