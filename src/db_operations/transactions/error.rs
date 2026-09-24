use my_no_sql_sdk::core::{
    db_json_entity::DbEntityParseFail, my_json::json_reader::JsonParseError,
};

use crate::db_operations::DbOperationError;

#[derive(Debug)]
pub enum TransactionOperationError {
    TransactionNotFound { id: String },
    DbEntityParseFail(DbEntityParseFail),
    DbOperationError(DbOperationError),
    SerdeJsonError(serde_json::Error),
    JsonParseError(JsonParseError),
}

impl From<JsonParseError> for TransactionOperationError {
    fn from(src: JsonParseError) -> Self {
        Self::JsonParseError(src)
    }
}

impl From<DbEntityParseFail> for TransactionOperationError {
    fn from(src: DbEntityParseFail) -> Self {
        Self::DbEntityParseFail(src)
    }
}

impl From<DbOperationError> for TransactionOperationError {
    fn from(src: DbOperationError) -> Self {
        Self::DbOperationError(src)
    }
}

impl From<serde_json::Error> for TransactionOperationError {
    fn from(src: serde_json::Error) -> Self {
        Self::SerdeJsonError(src)
    }
}
