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
pub struct DeletePartitionsInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(description = "Name of the table")]
    pub table_name: String,
    #[property(
        description = "Partition keys to remove (all rows under each partition will be deleted)"
    )]
    pub partition_keys: Vec<String>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct DeletePartitionsResponse {
    #[property(description = "Outcome message")]
    pub status: String,
    #[property(description = "Number of partitions submitted for deletion")]
    pub partitions_submitted: usize,
}

pub struct DeletePartitionsToolCallHandler {
    app: Arc<AppContext>,
}

impl DeletePartitionsToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for DeletePartitionsToolCallHandler {
    const FUNC_NAME: &'static str = "delete_partitions";

    const DESCRIPTION: &'static str = "\
Deletes whole partitions (and every row they contain) from a table in \
one call. Pass `table_name` and `partition_keys[]`. Requires MCP writes \
to be enabled by the admin in the UI Settings page (10-minute window). \
If this fails as DISABLED, ask the user to enable MCP writes — do not \
retry in a loop. See prompt 'mcp_writes_enable_policy'.";
}

#[async_trait::async_trait]
impl McpToolCall<DeletePartitionsInputData, DeletePartitionsResponse>
    for DeletePartitionsToolCallHandler
{
    async fn execute_tool_call(
        &self,
        model: DeletePartitionsInputData,
    ) -> Result<DeletePartitionsResponse, String> {
        let db_namespace = self
            .app
            .get_existing_namespace(model.namespace.as_deref())
            .map_err(|err| format!("{:?}", err))?;

        if model.partition_keys.is_empty() {
            return Err("`partition_keys` is empty — nothing to delete.".into());
        }

        super::write_gate::ensure_mcp_writes_enabled(self.app.as_ref())?;

        let db_table =
            db_operations::read::table::get(self.app.as_ref(), &db_namespace, &model.table_name)
                .await
                .map_err(|err| format!("{:?}", err))?;

        let partitions_submitted = model.partition_keys.len();

        let event_src = EventSource::as_client_request(self.app.as_ref());
        let now = DateTimeAsMicroseconds::now();

        db_operations::write::delete_partitions(
            self.app.as_ref(),
            &db_namespace,
            &db_table,
            model.partition_keys.into_iter(),
            event_src,
            DataSynchronizationPeriod::Sec5.get_sync_moment(),
            now,
        )
        .await
        .map_err(|err| format!("{:?}", err))?;

        Ok(DeletePartitionsResponse {
            status: "ok".into(),
            partitions_submitted,
        })
    }
}
