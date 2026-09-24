use std::collections::HashMap;

use mcp_server_middleware::*;

pub struct McpWritesEnablePolicyPromptHandler;

impl PromptDefinition for McpWritesEnablePolicyPromptHandler {
    const PROMPT_NAME: &'static str = "mcp_writes_enable_policy";

    const DESCRIPTION: &'static str =
        "How MyNoSqlServer write tools are gated: the admin must enable MCP writes from the UI Settings page (10-minute window). Read before any delete_row/insert_or_replace_row/clean_table/delete_partitions/bulk_delete_rows call.";

    fn get_argument_descriptions() -> Vec<PromptArgumentDescription> {
        Vec::new()
    }
}

#[async_trait::async_trait]
impl McpPromptService for McpWritesEnablePolicyPromptHandler {
    async fn execute_prompt(
        &self,
        _model: &HashMap<String, String>,
    ) -> Result<PromptExecutionResult, String> {
        let body = r#"# MCP Writes Enable Policy

The write tools (`delete_row`, `bulk_delete_rows`, `insert_or_replace_row`,
`clean_table`, `delete_partitions`) are DISABLED by default. There is no
password. Instead, the admin must explicitly turn MCP writes ON from the
MyNoSqlServer UI:

> **UI → Settings → "MCP writes" card → click "Enable MCP writes".**

Once enabled, writes stay on for **10 minutes** and then auto-disable.
The admin can also click **Disable** to turn them off immediately. A
server restart always leaves MCP writes disabled.

## Rules

1. **Never assume writes are enabled.** Read-only tools (`get_rows`,
   `get_list_of_tables`) always work, but write tools may be off.
2. **If a write tool fails with "MCP write operations are currently
   DISABLED"**, do NOT retry in a loop. Tell the user to enable MCP
   writes in the UI Settings page, then continue once they confirm.
3. The 10-minute window can expire mid-task. If a later write fails after
   earlier ones succeeded, the window likely lapsed — ask the user to
   re-enable.
4. For deletes that span many partitions or large/sensitive batches,
   prefer the `paste_delete_via_ui` workflow instead."#;

        Ok(PromptExecutionResult {
            description: "How MCP writes are enabled (UI Settings, 10-minute window).".to_string(),
            message: body.to_string(),
        })
    }
}
