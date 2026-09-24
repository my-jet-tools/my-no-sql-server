use std::collections::BTreeMap;
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
pub struct BulkDeleteRowsInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(description = "Name of the table")]
    pub table_name: String,
    #[property(description = "Partition key — all rows are deleted from this partition")]
    pub partition_key: String,
    #[property(description = "Row keys to delete")]
    pub row_keys: Vec<String>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct BulkDeleteRowsResponse {
    #[property(description = "Outcome message")]
    pub status: String,
    #[property(
        description = "Number of row keys submitted for deletion (does not guarantee all existed)"
    )]
    pub rows_submitted: usize,
}

pub struct BulkDeleteRowsToolCallHandler {
    app: Arc<AppContext>,
}

impl BulkDeleteRowsToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for BulkDeleteRowsToolCallHandler {
    const FUNC_NAME: &'static str = "bulk_delete_rows";

    const DESCRIPTION: &'static str = "\
Deletes multiple rows from a single partition in one call. Pass \
`partition_key` and `row_keys[]`. Requires MCP writes to be enabled by \
the admin in the UI Settings page (10-minute window). If this fails as \
DISABLED, ask the user to enable MCP writes — do not retry in a loop. \
See prompt 'mcp_writes_enable_policy'. For deletes that span multiple \
partitions, or for large/sensitive batches, prefer the \
'paste_delete_via_ui' workflow instead — see that prompt.";
}

#[async_trait::async_trait]
impl McpToolCall<BulkDeleteRowsInputData, BulkDeleteRowsResponse>
    for BulkDeleteRowsToolCallHandler
{
    async fn execute_tool_call(
        &self,
        model: BulkDeleteRowsInputData,
    ) -> Result<BulkDeleteRowsResponse, String> {
        let db_namespace = self
            .app
            .get_existing_namespace(model.namespace.as_deref())
            .map_err(|err| format!("{:?}", err))?;

        if model.row_keys.is_empty() {
            return Err("`row_keys` is empty — nothing to delete.".into());
        }

        super::write_gate::ensure_mcp_writes_enabled(self.app.as_ref())?;

        let db_table =
            db_operations::read::table::get(self.app.as_ref(), &db_namespace, &model.table_name)
                .await
                .map_err(|err| format!("{:?}", err))?;

        let rows_submitted = model.row_keys.len();
        let mut rows_to_delete: BTreeMap<String, Vec<String>> = BTreeMap::new();
        rows_to_delete.insert(model.partition_key, model.row_keys);

        let event_src = EventSource::as_client_request(self.app.as_ref());
        let now = DateTimeAsMicroseconds::now();

        db_operations::write::bulk_delete(
            self.app.as_ref(),
            &db_namespace,
            db_table.as_ref(),
            rows_to_delete.into_iter(),
            event_src,
            DataSynchronizationPeriod::Sec5.get_sync_moment(),
            now,
        )
        .await
        .map_err(|err| format!("{:?}", err))?;

        Ok(BulkDeleteRowsResponse {
            status: "ok".into(),
            rows_submitted,
        })
    }
}
