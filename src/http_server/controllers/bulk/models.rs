use std::collections::HashMap;

use my_http_server::macros::*;
use my_http_server::RawDataTyped;
use serde::{Deserialize, Serialize};

use crate::{
    db_sync::DataSynchronizationPeriod,
    http_server::controllers::row_controller::models::BaseDbRowContract,
};

#[derive(MyHttpInput)]
pub struct BulkDeleteInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_body_raw(
        description = "PartitionToDelete1:[RowToDelete1, RowToDelete2, RowToDelete3],[PartitionToDelete1]:[RowToDelete1, RowToDelete2, RowToDelete3]"
    )]
    pub body: RawDataTyped<HashMap<String, Vec<BaseDbRowContract>>>,
}

#[derive(MyHttpInput)]
pub struct BulkDeleteIfInputContract {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,

    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_body_raw(
        description = "Rows to delete: [{\"PartitionKey\":\"pk\",\"RowKey\":\"rk\",\"TimeStamp\":\"2026-08-09T16:44:39.5404\"}]. A row is deleted only when the TimeStamp stored in the table is still the one sent here"
    )]
    pub body: RawDataTyped<Vec<DeleteIfRowContract>>,
}

/// One row of a conditional bulk delete: which row, and the version the client read
/// it at. Unlike the entities of the write operations this is not a DbRow - only the
/// three fields which identify a row and its version travel here.
#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct DeleteIfRowContract {
    #[serde(rename = "PartitionKey")]
    pub partition_key: String,

    #[serde(rename = "RowKey")]
    pub row_key: String,

    #[serde(rename = "TimeStamp")]
    pub time_stamp: String,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct BulkDeleteIfResponseContract {
    /// How many rows really left the table.
    pub deleted: usize,

    /// The requested rows which were left in place, each with the reason why.
    pub skipped: Vec<SkippedDeleteIfRowContract>,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct SkippedDeleteIfRowContract {
    #[serde(rename = "PartitionKey")]
    pub partition_key: String,

    #[serde(rename = "RowKey")]
    pub row_key: String,

    /// `TimeStampMismatch` - the row is there but it is not the version the client
    /// read; `NotFound` - there is no such row at all.
    #[serde(rename = "Reason")]
    pub reason: String,
}

#[derive(MyHttpInput)]
pub struct CleanAndBulkInsertInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "partitionKey"; description = "Partition Key to clean before bulk insert operation";)]
    pub partition_key: Option<String>,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_query(
        name = "useTimestamp";
        description = "If true - use each entity's own TimeStamp (every entity must carry a valid one, otherwise the request fails with 400). If absent or false - the server assigns its own clock as before"
    )]
    pub use_timestamp: Option<bool>,

    #[http_body_raw(description = "DbRows")]
    pub body: RawDataTyped<Vec<BaseDbRowContract>>,
}

#[derive(MyHttpInput)]
pub struct CleanAndBulkInsertByChunksInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "partitionKey"; description = "Partition Key to clean before bulk insert operation. Taken into account only when a new process is started";)]
    pub partition_key: Option<String>,

    #[http_query(name = "processId"; description = "Id of the process. Omit it to start a new process - the id is returned in the response";)]
    pub process_id: Option<String>,

    #[http_query(
        name = "useTimestamp";
        description = "If true - use each entity's own TimeStamp (every entity of this chunk must carry a valid one, otherwise the chunk is rejected with 400). If absent or false - the server assigns its own clock as before"
    )]
    pub use_timestamp: Option<bool>,

    #[http_header(name = "session", description = "Writer session id")]
    pub session_id: Option<String>,

    #[http_body_raw(description = "DbRows")]
    pub body: RawDataTyped<Vec<BaseDbRowContract>>,
}

#[derive(MyHttpInput)]
pub struct InsertOrReplaceIfNewByChunksInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "processId"; description = "Id of the process. Omit it to start a new process - the id is returned in the response";)]
    pub process_id: Option<String>,

    #[http_header(name = "session", description = "Writer session id")]
    pub session_id: Option<String>,

    #[http_body_raw(
        description = "DbRows. Each row must carry a TimeStamp - on commit a row is written only when it is new or its TimeStamp is greater than the stored one"
    )]
    pub body: RawDataTyped<Vec<BaseDbRowContract>>,
}

#[derive(MyHttpInput)]
pub struct BulkProcessInputContract {
    #[http_query(name = "processId"; description = "Id of the process")]
    pub process_id: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_header(name = "session", description = "Writer session id")]
    pub session_id: Option<String>,
}

#[derive(MyHttpInput)]
pub struct CancelBulkProcessInputContract {
    #[http_query(name = "processId"; description = "Id of the process")]
    pub process_id: String,

    #[http_header(name = "session", description = "Writer session id")]
    pub session_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct BulkProcessResponse {
    #[serde(rename = "processId")]
    pub process_id: String,
}

#[derive(MyHttpInput)]
pub struct BulkInsertOrReplaceInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_query(
        name = "useTimestamp";
        description = "If true - use each entity's own TimeStamp (every entity must carry a valid one, otherwise the request fails with 400). If absent or false - the server assigns its own clock as before"
    )]
    pub use_timestamp: Option<bool>,

    #[http_body_raw(description = "Rows")]
    pub body: RawDataTyped<Vec<BaseDbRowContract>>,
}

#[derive(MyHttpInput)]
pub struct BulkInsertOrReplaceIfNewInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_body_raw(
        description = "Rows. Each row must carry a TimeStamp - a row is written only when it is new or its TimeStamp is greater than the stored one"
    )]
    pub body: RawDataTyped<Vec<BaseDbRowContract>>,
}
