use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use serde::*;

use crate::app::AppContext;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetListOfBackupsInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct BackupFileModel {
    #[property(description = "Snapshot file name. Use it to navigate into the backup")]
    pub file_name: String,
    #[property(description = "Size of the snapshot file in bytes")]
    pub size: i64,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetListOfBackupsResponse {
    #[property(description = "Amount of snapshot files")]
    pub count: usize,
    #[property(description = "List of snapshot files in the backup folder")]
    pub files: Vec<BackupFileModel>,
}

pub struct GetListOfBackupsToolCallHandler {
    app: Arc<AppContext>,
}

impl GetListOfBackupsToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for GetListOfBackupsToolCallHandler {
    const FUNC_NAME: &'static str = "get_list_of_backups";

    const DESCRIPTION: &'static str =
        "Returns the list of snapshot (backup) files available in the server's backup folder. Use the returned file_name to inspect tables, partitions and rows stored in a backup.";
}

#[async_trait::async_trait]
impl McpToolCall<GetListOfBackupsInputData, GetListOfBackupsResponse>
    for GetListOfBackupsToolCallHandler
{
    async fn execute_tool_call(
        &self,
        model: GetListOfBackupsInputData,
    ) -> Result<GetListOfBackupsResponse, String> {
        let db_namespace = self
            .app
            .get_existing_namespace(model.namespace.as_deref())
            .map_err(|err| format!("{:?}", err))?;

        let files =
            crate::operations::backup::get_list_of_files(self.app.as_ref(), &db_namespace).await;

        let files: Vec<BackupFileModel> = files
            .into_iter()
            .map(|file| BackupFileModel {
                file_name: file.name,
                size: file.size,
            })
            .collect();

        Ok(GetListOfBackupsResponse {
            count: files.len(),
            files,
        })
    }
}
