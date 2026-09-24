use my_http_server::macros::*;

use crate::db_sync::DataSynchronizationPeriod;

#[derive(MyHttpInput)]
pub struct CleanAndKeepMaxPartitionsAmountInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_query(name = "maxAmount"; description = "After operations there will be no more than maxPartitionsAmount partitions")]
    pub max_partitions_amount: usize,
}

#[derive(MyHttpInput)]
pub struct CleanPartitionAndKeepMaxRowsAmountInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "partitionKey"; description = "Partition which is going to cleaned")]
    pub partition_key: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_query(name = "maxAmount"; description = "After operations there will be no more than maxPartitionsAmount partitions")]
    pub max_amount: usize,
}
