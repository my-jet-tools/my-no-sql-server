use my_http_server::macros::*;
use my_http_server::RawDataTyped;
use serde::{Deserialize, Serialize};

#[derive(MyHttpInput)]
pub struct DataReaderGreetingInputModel {
    #[http_query(name = "name"; description = "Name of Application")]
    pub name: String,
    #[http_query(name = "version"; description = "Version of client library")]
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct DataReaderGreetingResult {
    #[serde(rename = "session")]
    pub session_id: String,
}

#[derive(MyHttpInput)]
pub struct SubscribeToTableInputModel {
    #[http_header(name = "session"; description = "Id of session")]
    pub session_id: String,

    #[http_query(name = "tableName"; description = "Table to subscriber")]
    pub table_name: String,
}

#[derive(MyHttpInput)]
pub struct PingInputModel {
    #[http_header(name = "session"; description = "Id of session")]
    pub session_id: String,
}

#[derive(MyHttpInput)]
pub struct GetChangesInputModel {
    #[http_header(name = "session"; description = "Id of session")]
    pub session_id: String,

    #[http_body_raw(description = "Update model")]
    pub body: RawDataTyped<GetChangesBodyModel>,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct GetChangesBodyModel {
    #[serde(rename = "uet")]
    pub update_expiration_time: Vec<UpdateExpirationDateTimeByTable>,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct UpdateExpirationDateTimeByTable {
    #[serde(rename = "tableName")]
    pub table_name: String,
    pub items: Vec<UpdateExpirationDateTime>,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct UpdateExpirationDateTime {
    #[serde(rename = "pk")]
    pub partition_key: String,
    #[serde(rename = "rk")]
    pub row_keys: Vec<String>,
    #[serde(rename = "ret")]
    pub set_db_rows_expiration_time: Option<String>,
    #[serde(rename = "pet")]
    pub set_db_partition_expiration_time: Option<String>,
}
