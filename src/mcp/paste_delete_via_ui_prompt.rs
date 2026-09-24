use std::collections::HashMap;

use mcp_server_middleware::*;

pub struct PasteDeleteViaUiPromptHandler;

impl PromptDefinition for PasteDeleteViaUiPromptHandler {
    const PROMPT_NAME: &'static str = "paste_delete_via_ui";

    const DESCRIPTION: &'static str =
        "Workflow: when a delete spans many partitions or a large/sensitive batch, do NOT loop `bulk_delete_rows` per partition. Emit a JSON array of {PartitionKey, RowKey} pairs in the chat reply and tell the user to paste it into the MyNoSqlServer UI's \"Paste & delete\" dialog.";

    fn get_argument_descriptions() -> Vec<PromptArgumentDescription> {
        Vec::new()
    }
}

#[async_trait::async_trait]
impl McpPromptService for PasteDeleteViaUiPromptHandler {
    async fn execute_prompt(
        &self,
        _model: &HashMap<String, String>,
    ) -> Result<PromptExecutionResult, String> {
        let body = r#"# Paste & delete via UI workflow

Use this workflow whenever a delete operation is *complex* — defined as
**any** of the following:

- The rows to delete live in **more than one partition** of the same
  table.
- The total batch is large (rule of thumb: more than ~50 rows) or
  sensitive enough that you would rather have the user inspect the full
  list before any data is touched.
- You don't want to take direct responsibility for the destructive call
  (e.g. the user asked you to "find rows to delete" rather than "delete
  them").

## What you do instead of calling `bulk_delete_rows`

1. Determine the table and the rows to delete (using `get_rows`,
   `get_list_of_tables`, or other read tools).
2. **Emit a plain JSON array** in the chat reply, one object per row,
   using EXACTLY these field names:

   ```json
   [
     { "PartitionKey": "<pk-1>", "RowKey": "<rk-1>" },
     { "PartitionKey": "<pk-1>", "RowKey": "<rk-2>" },
     { "PartitionKey": "<pk-2>", "RowKey": "<rk-3>" }
   ]
   ```

   - The array can mix multiple partitions of the **same table**.
   - **TableName is NOT part of the array** — the UI takes it from the
     currently-selected table in the Data page. Tell the user which
     table to select in the UI before pasting.
   - No other fields are accepted — anything other than `PartitionKey`
     and `RowKey` is ignored or rejected by the UI parser.
3. Instruct the user: *"Open the MyNoSqlServer UI, switch to the Data
   page, select table `<table>`, click **Paste & delete**, paste the
   JSON above, click Parse, verify the row/partition counts, then click
   Delete."*
4. **Do not call `delete_row` or `bulk_delete_rows` yourself** unless
   the user explicitly tells you to skip the UI step.

## Why

- The user gets to see the full impact list before any rows are touched.
- One server round-trip via `POST /api/Bulk/Delete` deletes across all
  partitions, instead of N MCP calls.
- The destructive action is confirmed inside the trusted UI rather than
  attributed to the model.

## Enabling writes

The UI "Paste & delete" dialog runs inside the trusted UI session and is
not gated by the MCP-writes toggle. If you *do* decide to call
`delete_row` / `bulk_delete_rows` directly instead, MCP writes must first
be enabled by the admin in the UI Settings page — see prompt
`mcp_writes_enable_policy`."#;

        Ok(PromptExecutionResult {
            description:
                "When and how to hand off a complex delete to the UI's Paste & delete dialog."
                    .to_string(),
            message: body.to_string(),
        })
    }
}
