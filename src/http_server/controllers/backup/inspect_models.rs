use my_http_server::macros::*;

#[derive(MyHttpInput)]
pub struct SnapshotFileContract {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,
    #[http_query(name: "fileName"; description: "Snapshot file name")]
    pub file_name: String,
}

#[derive(MyHttpInput)]
pub struct SnapshotTableContract {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,
    #[http_query(name: "fileName"; description: "Snapshot file name")]
    pub file_name: String,

    #[http_query(name: "tableName"; description: "Table name inside the snapshot")]
    pub table_name: String,
}

#[derive(MyHttpInput)]
pub struct SnapshotPartitionContract {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,
    #[http_query(name: "fileName"; description: "Snapshot file name")]
    pub file_name: String,

    #[http_query(name: "tableName"; description: "Table name inside the snapshot")]
    pub table_name: String,

    #[http_query(name: "partitionKey"; description: "Partition key inside the snapshot table")]
    pub partition_key: String,
}
