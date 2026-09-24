use my_http_server::macros::*;
use my_http_server::RawDataTyped;

use serde::{Deserialize, Serialize};

use crate::{
    db_operations::UpdateStatistics, db_sync::DataSynchronizationPeriod,
    http_server::controllers::mappers::ToSetExpirationTime,
};

#[derive(MyHttpInput)]
pub struct RowsCountInputContract {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "partitionKey"; description = "Partition Key")]
    pub partition_key: Option<String>,
}

#[derive(MyHttpInput)]
pub struct InsertOrReplaceInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_body_raw(description = "DbEntity description")]
    pub body: RawDataTyped<BaseDbRowContract>,
}

#[derive(MyHttpInput)]
pub struct InsertOrReplaceIfNewInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_body_raw(
        description = "DbEntity description. Must carry a TimeStamp - the row is written only when it is new or its TimeStamp is greater than the stored one"
    )]
    pub body: RawDataTyped<BaseDbRowContract>,
}

#[derive(MyHttpInput)]
pub struct InsertInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_body_raw(description = "DbEntity description")]
    pub body: RawDataTyped<BaseDbRowContract>,
}

#[derive(MyHttpInput)]
pub struct ReplaceInputContract {
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,

    #[http_body_raw(description = "DbEntity description")]
    pub body: RawDataTyped<BaseDbRowContract>,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct BaseDbRowContract {
    #[serde(rename = "PartitionKey")]
    pub partition_key: String,

    #[serde(rename = "RowKey")]
    pub row_key: String,

    #[serde(rename = "TimeStamp")]
    pub time_stamp: String,

    #[serde(rename = "Expires")]
    pub expires: Option<String>,
}

#[derive(MyHttpInput)]
pub struct GetRowInputModel {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "partitionKey"; description = "Partition Key")]
    pub partition_key: Option<String>,

    #[http_query(name = "rowKey"; description = "Row Key")]
    pub row_key: Option<String>,

    #[http_query(name = "limit"; description = "Limit amount of records we are going to get")]
    pub limit: Option<usize>,

    #[http_query(name = "skip"; description = "Skip amount of records before start collecting them")]
    pub skip: Option<usize>,

    #[http_header(name ="updatePartitionLastReadTime"; description = "Update partition last read time")]
    pub update_partition_last_read_access_time: Option<bool>,

    #[http_header(name ="setPartitionExpirationTime"; description = "Set Partition Expiration time")]
    pub set_partition_expiration_time: Option<String>,

    #[http_header(name ="updateRowsLastReadTime"; description = "Update partition last read time")]
    pub update_db_rows_last_read_access_time: Option<bool>,

    #[http_header(name ="setRowsExpirationTime" description = "Set Found DbRows Expiration time")]
    pub set_db_rows_expiration_time: Option<String>,

    #[http_header(name = "x-compress"; description = "Response compression preference (e.g. \"zstd\")")]
    pub x_compress: Option<String>,
}

impl GetRowInputModel {
    pub fn get_update_statistics(&self) -> UpdateStatistics {
        UpdateStatistics {
            update_partition_last_read_access_time: if let Some(value) =
                self.update_partition_last_read_access_time
            {
                value
            } else {
                false
            },
            update_rows_last_read_access_time: if let Some(value) =
                self.update_db_rows_last_read_access_time
            {
                value
            } else {
                false
            },
            update_partition_expiration_time: self
                .set_partition_expiration_time
                .to_set_expiration_time(),
            update_rows_expiration_time: self.set_db_rows_expiration_time.to_set_expiration_time(),
        }
    }
}

#[derive(MyHttpInput)]
pub struct DeleteRowIfInputModel {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "partitionKey"; description = "Partition Key")]
    pub partition_key: String,

    #[http_query(name = "rowKey"; description = "Row Key")]
    pub row_key: String,

    #[http_query(
        name = "timeStamp";
        description = "TimeStamp the row was read at. The row is deleted only when this is still the TimeStamp stored in the table"
    )]
    pub time_stamp: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,
}

#[derive(MyHttpInput)]
pub struct DeleteRowInputModel {
    #[http_header(name = "ns"; description = "Namespace to work in. Empty or absent means the default namespace")]
    pub namespace: Option<String>,
    #[http_query(name = "tableName"; description = "Name of a table")]
    pub table_name: String,

    #[http_query(name = "partitionKey"; description = "Partition Key")]
    pub partition_key: String,

    #[http_query(name = "rowKey"; description = "Row Key")]
    pub row_key: String,

    #[http_query(name = "syncPeriod"; description = "Synchronization period"; default)]
    pub sync_period: DataSynchronizationPeriod,
}
