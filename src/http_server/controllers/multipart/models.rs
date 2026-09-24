use my_http_server::macros::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct NewMultipartResponse {
    #[serde(rename = "snapshotId")]
    pub snapshot_id: String,
}

#[derive(MyHttpInput)]
pub struct NewMultipartInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,
}

#[derive(MyHttpInput)]
pub struct NextMultipartRequestInputContract {
    #[http_query(name = "requestId"; description = "Id of request")]
    pub request_id: i64,

    #[http_query(name = "maxRecordsCount"; description = "Chunk size")]
    pub max_records_count: usize,
}
