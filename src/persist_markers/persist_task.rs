use std::sync::Arc;

use my_no_sql_sdk::core::db::{DbRow, DbTableName, PartitionKey};

pub struct SyncRowJobDescription {
    pub partition_key: PartitionKey,
    pub items: Vec<Arc<DbRow>>,
}

pub enum PersistTask {
    SaveTableAttributes(DbTableName),
    SyncTable(DbTableName),
    SyncPartition {
        table_name: DbTableName,
        partition_key: PartitionKey,
    },
    SyncRows {
        table_name: DbTableName,
        jobs: Vec<SyncRowJobDescription>,
    },
}
