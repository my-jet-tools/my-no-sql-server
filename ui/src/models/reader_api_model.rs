use serde::*;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ReaderApiModel {
    pub id: String,
    pub name: String,
    #[serde(default = "crate::models::default_namespace")]
    pub namespace: String,
    pub ip: String,
    #[serde(default)]
    pub tables: Vec<String>,
    #[serde(rename = "lastIncomingTime")]
    pub last_incoming_time: String,
    #[serde(rename = "connectedTime")]
    pub connected_time: String,
    #[serde(rename = "pendingToSend")]
    pub pending_to_send: u64,
    #[serde(rename = "sentPerSecond", default)]
    pub sent_per_second: Vec<u64>,
    #[serde(rename = "isNode", default)]
    pub is_node: bool,
    /// Round trip in microseconds, as the client reported it with `PingWithLatency`.
    /// `None` until it does — HTTP readers and older SDKs never do.
    #[serde(default)]
    pub latency: Option<i64>,
}
