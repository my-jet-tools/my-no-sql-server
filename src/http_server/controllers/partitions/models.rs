use my_http_server::macros::*;
use serde_derive::Serialize;

#[derive(MyHttpInput)]
pub struct GetPartitionsAmountContract {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,
    #[http_query(name: "tableName"; description: "Name of a table")]
    pub table_name: String,
}

#[derive(MyHttpInput)]
pub struct GetPartitionsListContract {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,
    #[http_query(name: "tableName"; description: "Name of a table")]
    pub table_name: String,

    #[http_query(name: "skip"; description: "Skip amount before we start return")]
    pub skip: Option<usize>,

    #[http_query(name: "limit"; description: "Maximum records to return")]
    pub limit: Option<usize>,
}

#[derive(MyHttpObjectStructure, Serialize)]
pub struct PartitionsHttpResult {
    pub amount: usize,
    pub data: Vec<String>,
}

#[derive(MyHttpObjectStructure, Serialize)]
pub struct PartitionMetricHttpModel {
    #[serde(rename = "partitionKey")]
    pub partition_key: String,
    #[serde(rename = "recordsCount")]
    pub records_count: usize,
    #[serde(rename = "dataSize")]
    pub data_size: usize,
}
