use std::sync::Arc;

use mcp_server_middleware::*;
use my_ai_agent::macros::ApplyJsonSchema;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use serde::*;

use crate::app::AppContext;

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetTableClientsInputData {
    #[property(description = "Optional namespace. Empty means the default namespace")]
    pub namespace: Option<String>,
    #[property(description = "Name of the table to look up the clients of")]
    pub table_name: String,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct TableWriterModel {
    #[property(description = "Session id the server issued to this writer")]
    pub session: String,
    #[property(description = "Name the writer introduced itself with in its ping")]
    pub name: String,
    #[property(description = "Version of the client the writer runs")]
    pub version: String,
    #[property(description = "Address the writer's requests come from")]
    pub addr: String,
    #[property(
        description = "Seconds since this writer's last ping. A writer silent for more than a minute is dropped from the list altogether, so this is always under 60"
    )]
    pub last_ping_seconds_ago: i64,
    #[property(
        description = "Write requests per second this writer is doing right now - counted across ALL of its tables, not only this one. Zero means it is connected but idle"
    )]
    pub requests_per_second: usize,
    #[property(
        description = "Bytes per second this writer is sending right now - counted across ALL of its tables, not only this one"
    )]
    pub bytes_per_second: usize,
    #[property(description = "Other tables this writer declared it writes to")]
    pub also_writes_to: Vec<String>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct TableReaderModel {
    #[property(description = "Session id of the reader connection")]
    pub session: String,
    #[property(description = "Name the reader introduced itself with")]
    pub name: String,
    #[property(description = "Address the reader is connected from")]
    pub ip: String,
    #[property(
        description = "True when this is another MyNoSqlServer node mirroring the table rather than an application reader"
    )]
    pub is_node: bool,
    #[property(
        description = "Payloads waiting to be sent to this reader. A growing value means the reader is not keeping up with the updates"
    )]
    pub pending_to_send: usize,
    #[property(
        description = "Bytes per second the server is sending to this reader right now - counted across ALL of its subscriptions, not only this table"
    )]
    pub outgoing_bytes_per_second: usize,
    #[property(
        description = "Round trip to this reader in microseconds, as the reader measured it with its keep-alive ping and reported to the server. Null until it reports one - HTTP readers and clients on an SDK which predates the latency ping never do"
    )]
    pub latency: Option<i64>,
    #[property(description = "Other tables this reader is subscribed to")]
    pub also_reads: Vec<String>,
}

#[derive(ApplyJsonSchema, Debug, Serialize, Deserialize)]
pub struct GetTableClientsResponse {
    #[property(description = "Table the answer is about")]
    pub table_name: String,
    #[property(description = "Namespace the table was looked up in")]
    pub namespace: String,
    #[property(
        description = "False when no such table exists in this namespace. Writers can still be listed: a writer declares the tables it intends to work with, and a table is created by its first write"
    )]
    pub table_exists: bool,
    #[property(description = "Amount of writers of this table")]
    pub writers_amount: usize,
    #[property(description = "Amount of readers of this table")]
    pub readers_amount: usize,
    #[property(description = "Clients which write this table")]
    pub writers: Vec<TableWriterModel>,
    #[property(description = "Clients which read this table")]
    pub readers: Vec<TableReaderModel>,
}

pub struct GetTableClientsToolCallHandler {
    app: Arc<AppContext>,
}

impl GetTableClientsToolCallHandler {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

impl ToolDefinition for GetTableClientsToolCallHandler {
    const FUNC_NAME: &'static str = "get_table_clients";

    const DESCRIPTION: &'static str = "Returns who writes a MyNoSql table and who reads it RIGHT NOW - use it to find out who owns a table, who would be affected by changing or deleting it, and whether anybody is still using it at all. \
         Writers come from the ping handshake and are self-declared: a writer names the tables it works with, and it drops off this list about a minute after it stops pinging. So a writer which is not running now, one which writes without pinging, and a write done over gRPC, the UI or MCP are all invisible here. \
         Readers are the live subscriptions of the connected readers, other MyNoSqlServer nodes included. \
         Both lists are scoped to the namespace and describe the present moment, not history: an empty answer means nobody is connected for that table now, NOT that the table is unused.";
}

#[async_trait::async_trait]
impl McpToolCall<GetTableClientsInputData, GetTableClientsResponse>
    for GetTableClientsToolCallHandler
{
    async fn execute_tool_call(
        &self,
        model: GetTableClientsInputData,
    ) -> Result<GetTableClientsResponse, String> {
        let db_namespace = self
            .app
            .get_existing_namespace(model.namespace.as_deref())
            .map_err(|err| format!("{:?}", err))?;

        let table_name = model.table_name.as_str();

        let now = DateTimeAsMicroseconds::now();

        // Traffic lives in a map of its own, keyed by the very session id the
        // writers are keyed by: a writer with no entry in it had no traffic in
        // the last completed second, which is a zero rate, not a missing writer.
        let writers_traffic = self.app.writers_traffic.get_snapshot();

        let writers: Vec<TableWriterModel> = self
            .app
            .http_writers
            .get(|session_id, info| {
                // Table names are namespace-scoped, and two namespaces may each
                // hold one of the same name - a writer of the other namespace's
                // table is not a writer of this one.
                if info.namespace.as_str() != db_namespace.name.as_str() {
                    return None;
                }

                if !info.tables.iter().any(|itm| itm.as_str() == table_name) {
                    return None;
                }

                let (requests_per_second, bytes_per_second) =
                    writers_traffic.get(session_id).copied().unwrap_or((0, 0));

                Some(TableWriterModel {
                    session: session_id.to_string(),
                    name: info.name.to_string(),
                    version: info.version.to_string(),
                    addr: info.addr.to_string(),
                    last_ping_seconds_ago: now
                        .duration_since(info.last_ping)
                        .as_positive_or_zero()
                        .as_secs() as i64,
                    requests_per_second,
                    bytes_per_second,
                    also_writes_to: info
                        .tables
                        .iter()
                        .filter(|itm| itm.as_str() != table_name)
                        .map(|itm| itm.to_string())
                        .collect(),
                })
            })
            .await
            .into_iter()
            .flatten()
            .collect();

        let mut readers = Vec::new();

        if let Some(subscribed) = self
            .app
            .data_readers
            .get_subscribed_to_table(&db_namespace.name, table_name)
            .await
        {
            for data_reader in subscribed {
                let metrics = data_reader.get_metrics().await;
                let (_, outgoing_bytes_per_second) = data_reader.get_traffic_per_second();

                readers.push(TableReaderModel {
                    session: metrics.session_id,
                    name: metrics.name,
                    ip: metrics.ip,
                    is_node: data_reader.is_node(),
                    pending_to_send: metrics.pending_to_send,
                    outgoing_bytes_per_second,
                    latency: metrics.latency,
                    also_reads: metrics
                        .tables
                        .into_iter()
                        .filter(|itm| itm.as_str() != table_name)
                        .collect(),
                });
            }
        }

        Ok(GetTableClientsResponse {
            table_name: table_name.to_string(),
            namespace: db_namespace.name.to_string(),
            table_exists: db_namespace.db.get_table(table_name).is_some(),
            writers_amount: writers.len(),
            readers_amount: readers.len(),
            writers,
            readers,
        })
    }
}
