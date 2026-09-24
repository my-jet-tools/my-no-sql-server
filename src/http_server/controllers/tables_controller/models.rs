use my_http_server::macros::*;
use my_no_sql_sdk::server::DbTable;
use serde::{Deserialize, Serialize};

use crate::db_sync::DataSynchronizationPeriod;

#[derive(MyHttpInput)]
pub struct GetTableSizeContract {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,
    #[http_query(name: "tableName"; description: "Name of a table")]
    pub table_name: String,
}

#[derive(MyHttpInput)]
pub struct CleanTableContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,
    #[http_query(name: "syncPeriod"; description: "Synchronization period")]
    pub sync_period: DataSynchronizationPeriod,
}

#[derive(MyHttpInput)]
pub struct UpdatePersistTableContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(description = "Persist table"; default: true)]
    pub persist: bool,
}

#[derive(MyHttpInput)]
pub struct UpdateCompressedTableContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(description = "Keep the rows of this table compressed in memory"; default: false)]
    pub compressed: bool,

    #[http_query(name: "forceCompress"; description = "If true - immediately re-encode all already stored rows to match the flag (otherwise only new/rewritten rows are affected)"; default: false)]
    pub force_compress: bool,
}

#[derive(Deserialize, Serialize, MyHttpObjectStructure)]
pub struct TableContract {
    pub name: String,
    pub persist: bool,
    #[serde(rename = "maxPartitionsAmount")]
    pub max_partitions_amount: Option<usize>,
    #[serde(rename = "maxRowsPerPartitionAmount")]
    pub max_rows_per_partition_amount: Option<usize>,
    pub compressed: bool,
}

impl TableContract {
    pub fn from_table_wrapper(table_wrapper: &DbTable) -> TableContract {
        let table_snapshot = table_wrapper.get_attributes();
        TableContract {
            name: table_wrapper.name.to_string(),
            persist: table_snapshot.persist,
            max_partitions_amount: table_snapshot.max_partitions_amount,
            max_rows_per_partition_amount: table_snapshot.max_rows_per_partition_amount,
            compressed: table_snapshot.compressed,
        }
    }
}

#[derive(MyHttpInput)]
pub struct CreateTableContract {
    #[http_query(name: "tableName"; description: "Name of a table")]
    pub table_name: String,

    #[http_query(description: "Persist table"; default: true)]
    pub persist: bool,

    #[http_query(name: "maxPartitionsAmount"; description: "Maximum partitions amount. Empty - means unlimited")]
    pub max_partitions_amount: Option<usize>,

    #[http_query(name: "maxRowsPerPartitionAmount"; description: "Maximum rows per partition amount. Empty - means unlimited")]
    pub max_rows_per_partition_amount: Option<usize>,

    #[http_query(description: "Keep the rows of this table compressed in memory"; default: false)]
    pub compressed: bool,

    #[http_query(name: "syncPeriod"; description: "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,
}

#[derive(MyHttpInput)]
pub struct TableMigrationInputContract {
    #[http_query(name = "remoteUrl"; description = "Url of the remote MyNoSqlServer we are going to copy data from")]
    pub remote_url: String,

    #[http_query(name = "tableName"; description = "Table name of the current MyNoSqlServer we are going to copy data to")]
    pub table_name: String,

    #[http_query(name = "remoteTableName"; description = "Table name of the remote MyNoSqlServer we are going to copy data from")]
    pub remote_table_name: String,
}

#[derive(MyHttpInput)]
pub struct DeleteTableContract {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,
    #[http_header(name = "apikey"; description = "Api Key protecting the table to be deleted")]
    pub api_key: String,
}
