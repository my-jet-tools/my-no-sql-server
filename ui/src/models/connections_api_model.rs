use serde::*;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ConnectionReaderApiModel {
    pub id: String,
    pub name: String,
    #[serde(default = "crate::models::default_namespace")]
    pub namespace: String,
    pub ip: String,
    #[serde(rename = "incomingPerSecond", default)]
    pub incoming_per_second: u64,
    #[serde(rename = "outgoingPerSecond", default)]
    pub outgoing_per_second: u64,
    #[serde(rename = "pendingToSend", default)]
    pub pending_to_send: u64,
    #[serde(rename = "isNode", default)]
    pub is_node: bool,
    /// Round trip in microseconds, as the client reported it. `None` until it does.
    #[serde(default)]
    pub latency: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ConnectionWriterApiModel {
    #[serde(default)]
    pub session: String,
    pub name: String,
    #[serde(default = "crate::models::default_namespace")]
    pub namespace: String,
    pub version: String,
    #[serde(default)]
    pub addr: String,
    #[serde(default)]
    pub tables: Vec<String>,
    #[serde(rename = "lastIncomingTime", default)]
    pub last_incoming_time: String,
    #[serde(rename = "reqPerSecond", default)]
    pub req_per_second: u64,
    #[serde(rename = "bytesPerSecond", default)]
    pub bytes_per_second: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ConnectionsApiModel {
    #[serde(rename = "incomingPerSecond", default)]
    pub incoming_per_second: u64,
    #[serde(rename = "outgoingPerSecond", default)]
    pub outgoing_per_second: u64,
    #[serde(rename = "writePayloadsPerSecond", default)]
    pub write_payloads_per_second: u64,
    #[serde(rename = "writeBytesPerSecond", default)]
    pub write_bytes_per_second: u64,
    #[serde(default)]
    pub readers: Vec<ConnectionReaderApiModel>,
    #[serde(default)]
    pub writers: Vec<ConnectionWriterApiModel>,
}
