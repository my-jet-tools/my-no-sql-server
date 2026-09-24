use crate::app::AppContext;
use my_http_server::macros::*;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct ConnectionReaderModel {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub ip: String,
    #[serde(rename = "incomingPerSecond")]
    pub incoming_per_second: usize,
    #[serde(rename = "outgoingPerSecond")]
    pub outgoing_per_second: usize,
    #[serde(rename = "pendingToSend")]
    pub pending_to_send: usize,
    #[serde(rename = "isNode")]
    pub is_node: bool,
    /// Round trip in microseconds, as the reader reported it. Null until it does.
    pub latency: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct ConnectionWriterModel {
    pub session: String,
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub addr: String,
    pub tables: Vec<String>,
    #[serde(rename = "lastIncomingTime")]
    pub last_incoming_time: String,
    #[serde(rename = "reqPerSecond")]
    pub req_per_second: usize,
    #[serde(rename = "bytesPerSecond")]
    pub bytes_per_second: usize,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct ConnectionsModel {
    #[serde(rename = "incomingPerSecond")]
    pub incoming_per_second: usize,
    #[serde(rename = "outgoingPerSecond")]
    pub outgoing_per_second: usize,
    #[serde(rename = "writePayloadsPerSecond")]
    pub write_payloads_per_second: usize,
    #[serde(rename = "writeBytesPerSecond")]
    pub write_bytes_per_second: usize,
    pub readers: Vec<ConnectionReaderModel>,
    pub writers: Vec<ConnectionWriterModel>,
}

impl ConnectionsModel {
    pub async fn new(app: &AppContext) -> Self {
        let mut readers = Vec::new();
        let mut total_incoming = 0;
        let mut total_outgoing = 0;

        for data_reader in app.data_readers.get_all().await {
            let (incoming, outgoing) = data_reader.get_traffic_per_second();
            total_incoming += incoming;
            total_outgoing += outgoing;

            let metrics = data_reader.get_metrics().await;

            readers.push(ConnectionReaderModel {
                id: metrics.session_id,
                name: metrics.name,
                namespace: data_reader.get_namespace().to_string(),
                ip: metrics.ip,
                incoming_per_second: incoming,
                outgoing_per_second: outgoing,
                pending_to_send: metrics.pending_to_send,
                is_node: data_reader.is_node(),
                latency: metrics.latency,
            });
        }

        let now = DateTimeAsMicroseconds::now();
        let writers_traffic = app.writers_traffic.get_snapshot();
        let writers = app
            .http_writers
            .get(|session_id, info| {
                let (req_per_second, bytes_per_second) =
                    writers_traffic.get(session_id).copied().unwrap_or((0, 0));
                ConnectionWriterModel {
                    session: session_id.to_string(),
                    name: info.name.to_string(),
                    namespace: info.namespace.to_string(),
                    version: info.version.to_string(),
                    addr: info.addr.to_string(),
                    tables: info.tables.clone(),
                    last_incoming_time: format!(
                        "{:?}",
                        now.duration_since(info.last_ping).as_positive_or_zero()
                    ),
                    req_per_second,
                    bytes_per_second,
                }
            })
            .await;

        Self {
            incoming_per_second: total_incoming,
            outgoing_per_second: total_outgoing,
            write_payloads_per_second: app.write_payloads_per_second.get_value(),
            write_bytes_per_second: app.write_bytes_per_second.get_value(),
            readers,
            writers,
        }
    }
}
