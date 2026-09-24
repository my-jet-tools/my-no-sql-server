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
pub struct BulkInsertOrReplaceRowsInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(description = "Name of the table")]
    pub table_name: String,
    #[property(
        description = "JSON array of row objects. Each object must include 'PartitionKey' and 'RowKey' string fields plus any additional row data. Rows may span multiple partitions — they are grouped by PartitionKey automatically."
    )]
    pub entities_json: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct BulkInsertOrReplaceRowsResponse {
    #[property(description = "Outcome message")]
    pub status: String,
    #[property(description = "Number of rows submitted")]
    pub rows_submitted: usize,
    #[property(description = "Number of distinct partitions affected")]
    pub partitions_affected: usize,
}

pub struct BulkInsertOrReplaceRowsToolCallHandler {
    app: Arc<AppContext>,
}

impl BulkInsertOrReplaceRowsToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for BulkInsertOrReplaceRowsToolCallHandler {
    const FUNC_NAME: &'static str = "bulk_insert_or_replace_rows";

    const DESCRIPTION: &'static str = "\
Inserts or replaces multiple rows in one call. Pass `entities_json` as a \
JSON array of full row objects; each must include 'PartitionKey' and \
'RowKey'. Rows may belong to different partitions — they are grouped \
automatically. Existing rows with the same PartitionKey + RowKey are \
replaced. Prefer this over calling 'insert_or_replace_row' in a loop. \
Requires MCP writes to be enabled by the admin in the UI Settings page \
(10-minute window). If this fails as DISABLED, ask the user to enable MCP \
writes — do not retry in a loop. See prompt 'mcp_writes_enable_policy'.";
}

#[async_trait::async_trait]
impl McpToolCall<BulkInsertOrReplaceRowsInputData, BulkInsertOrReplaceRowsResponse>
    for BulkInsertOrReplaceRowsToolCallHandler
{
    async fn execute_tool_call(
        &self,
        model: BulkInsertOrReplaceRowsInputData,
    ) -> Result<BulkInsertOrReplaceRowsResponse, String> {
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

        let rows_by_partition = db_operations::parse_json_entity::parse_grouped_by_partition_key(
            model.entities_json.as_bytes(),
            &now,
        )
        .map_err(|err| format!("{:?}", err))?;

        if rows_by_partition.is_empty() {
            return Err("`entities_json` contains no rows — nothing to insert.".into());
        }

        let partitions_affected = rows_by_partition.len();
        let rows_submitted = rows_by_partition.iter().map(|(_, rows)| rows.len()).sum();

        db_operations::write::bulk_insert_or_update::execute(
            self.app.as_ref(),
            &db_namespace,
            &db_table,
            rows_by_partition,
            event_src,
            DataSynchronizationPeriod::Sec5.get_sync_moment(),
            now.date_time,
        )
        .await
        .map_err(|err| format!("{:?}", err))?;

        Ok(BulkInsertOrReplaceRowsResponse {
            status: "ok".into(),
            rows_submitted,
            partitions_affected,
        })
    }
}
