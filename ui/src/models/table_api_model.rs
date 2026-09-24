use serde::*;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TableApiModel {
    pub name: String,
    pub persist: bool,
    /// Whether the rows of this table are kept compressed in memory. Defaulted
    /// so the UI still parses a status from a server that predates the flag.
    #[serde(default)]
    pub compressed: bool,
    #[serde(rename = "maxPartitionsAmount")]
    pub max_partitions_amount: Option<u64>,
    #[serde(rename = "maxRowsPerPartition")]
    pub max_rows_per_partition: Option<u64>,
    #[serde(rename = "partitionsCount")]
    pub partitions_count: u64,
    #[serde(rename = "dataSize")]
    pub data_size: u64,
    #[serde(rename = "recordsAmount")]
    pub records_amount: u64,
    #[serde(rename = "expirationIndex")]
    pub expiration_index_records_amount: u64,
    #[serde(rename = "lastUpdateTime")]
    pub last_update_time: i64,
    #[serde(rename = "lastPersistTime")]
    pub last_persist_time: Option<i64>,
    #[serde(rename = "lastPersistDuration", default)]
    pub last_persist_duration: Vec<u64>,
    #[serde(rename = "nextPersistTime")]
    pub next_persist_time: Option<i64>,
    #[serde(rename = "persistAmount")]
    pub persist_amount: u64,
    #[serde(rename = "avgEntitySize")]
    pub avg_entity_size: u64,
}
