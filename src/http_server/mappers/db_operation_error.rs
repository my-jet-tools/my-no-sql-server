use crate::db_operations::DbOperationError;

use my_http_server::{HttpFailResult, HttpOutput, WebContentType};
use my_no_sql_sdk::core::db_json_entity::DbEntityParseFail;
use my_no_sql_sdk::core::my_json::json_reader::JsonParseError;

use super::{OperationFailHttpContract, OperationFailReason};

pub const OPERATION_FAIL_HTTP_STATUS_CODE: u16 = 400;

impl From<DbOperationError> for HttpFailResult {
    fn from(src: DbOperationError) -> Self {
        match src {
            DbOperationError::TableAlreadyExists => {
                let err_model = OperationFailHttpContract {
                    reason: OperationFailReason::TableAlreadyExists,
                    message: format!("Table already exists"),
                };
                let content = serde_json::to_vec(&err_model).unwrap();

                HttpOutput::Content {
                    headers: WebContentType::Json.into(),
                    status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                    content,
                }
                .into_http_fail_result(true, true)
            }
            DbOperationError::TableNotFound(table_name) => {
                super::super::get_table::table_not_found_http_result(table_name.as_str())
            }
            DbOperationError::RecordNotFound => HttpOutput::Content {
                headers: WebContentType::Json.into(),
                status_code: 404,
                content: format!("Record not found").into_bytes(),
            }
            .into_http_fail_result(false, false),
            DbOperationError::ApplicationIsNotInitializedYet => HttpOutput::Content {
                headers: WebContentType::Json.into(),
                status_code: 503,
                content: format!("Application is not initialized yet").into_bytes(),
            }
            .into_http_fail_result(false, false),
            DbOperationError::OptimisticConcurrencyUpdateFails => HttpOutput::Content {
                headers: WebContentType::Json.into(),
                status_code: 409,
                content: format!("Record is changed").into_bytes(),
            }
            .into_http_fail_result(false, false),
            DbOperationError::RecordAlreadyExists => {
                let err_model = OperationFailHttpContract {
                    reason: OperationFailReason::RecordAlreadyExists,
                    message: format!("Record already exists"),
                };
                let content = serde_json::to_vec(&err_model).unwrap();

                HttpOutput::Content {
                    headers: WebContentType::Json.into(),
                    status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                    content,
                }
                .into_http_fail_result(false, false)
            }
            DbOperationError::TimeStampFieldRequires => {
                let err_model = OperationFailHttpContract {
                    reason: OperationFailReason::RequiredEntityFieldIsMissing,
                    message: format!("Timestamp field requires"),
                };

                let content = serde_json::to_vec(&err_model).unwrap();
                HttpOutput::Content {
                    headers: WebContentType::Text.into(),
                    status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                    content,
                }
                .into_http_fail_result(true, true)
            }
            DbOperationError::TableNameValidationError(reason) => {
                let err_model = OperationFailHttpContract {
                    reason: OperationFailReason::RequiredEntityFieldIsMissing,
                    message: format!("Invalid table name: {}", reason),
                };

                let content = serde_json::to_vec(&err_model).unwrap();
                HttpOutput::Content {
                    headers: WebContentType::Text.into(),
                    status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                    content,
                }
                .into_http_fail_result(true, true)
            }
            DbOperationError::NamespaceNotFound(namespace) => {
                let err_model = OperationFailHttpContract {
                    reason: OperationFailReason::NamespaceNotFound,
                    message: format!("Namespace '{}' not found", namespace),
                };

                let content = serde_json::to_vec(&err_model).unwrap();
                HttpOutput::Content {
                    headers: WebContentType::Json.into(),
                    status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                    content,
                }
                .into_http_fail_result(true, true)
            }
            DbOperationError::NamespaceNameValidationError(reason) => {
                let err_model = OperationFailHttpContract {
                    reason: OperationFailReason::RequiredEntityFieldIsMissing,
                    message: format!("Invalid namespace name: {}", reason),
                };

                let content = serde_json::to_vec(&err_model).unwrap();
                HttpOutput::Content {
                    headers: WebContentType::Text.into(),
                    status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                    content,
                }
                .into_http_fail_result(true, true)
            }
            DbOperationError::DbEntityParseFail(src) => {
                from_db_entity_parse_fail_to_http_result(src)
            }
        }
    }
}

pub fn from_json_parse_error_to_http_result(value: JsonParseError) -> HttpFailResult {
    let err_model = OperationFailHttpContract {
        reason: OperationFailReason::JsonParseFail,
        message: value.to_string(),
    };

    let content = serde_json::to_vec(&err_model).unwrap();

    HttpOutput::Content {
        headers: WebContentType::Json.into(),
        status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
        content,
    }
    .into_http_fail_result(true, true)
}

pub fn from_db_entity_parse_fail_to_http_result(src: DbEntityParseFail) -> HttpFailResult {
    match src {
        DbEntityParseFail::FieldPartitionKeyIsRequired => {
            let err_model = OperationFailHttpContract {
                reason: OperationFailReason::RequiredEntityFieldIsMissing,
                message: format!("PartitionKey field is required"),
            };

            let content = serde_json::to_vec(&err_model).unwrap();

            HttpOutput::Content {
                headers: WebContentType::Json.into(),
                status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                content,
            }
            .into_http_fail_result(true, true)
        }
        DbEntityParseFail::PartitionKeyIsTooLong => {
            let err_model = OperationFailHttpContract {
                reason: OperationFailReason::RequiredEntityFieldIsMissing,
                message: format!("PartitionKey is too long"),
            };

            let content = serde_json::to_vec(&err_model).unwrap();

            HttpOutput::Content {
                headers: WebContentType::Json.into(),
                status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                content,
            }
            .into_http_fail_result(true, true)
        }
        DbEntityParseFail::FieldRowKeyIsRequired => {
            let err_model = OperationFailHttpContract {
                reason: OperationFailReason::RequiredEntityFieldIsMissing,
                message: format!("RowKey field is required"),
            };

            let content = serde_json::to_vec(&err_model).unwrap();

            HttpOutput::Content {
                headers: WebContentType::Json.into(),
                status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                content,
            }
            .into_http_fail_result(true, true)
        }

        DbEntityParseFail::JsonParseError(json_parse_error) => {
            from_json_parse_error_to_http_result(json_parse_error)
        }
        DbEntityParseFail::FieldPartitionKeyCanNotBeNull => {
            let err_model = OperationFailHttpContract {
                reason: OperationFailReason::RequiredEntityFieldIsMissing,
                message: format!("PartitionKey can not be null"),
            };

            let content = serde_json::to_vec(&err_model).unwrap();

            HttpOutput::Content {
                headers: WebContentType::Json.into(),
                status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                content,
            }
            .into_http_fail_result(true, true)
        }
        DbEntityParseFail::FieldRowKeyCanNotBeNull => {
            let err_model = OperationFailHttpContract {
                reason: OperationFailReason::RequiredEntityFieldIsMissing,
                message: format!("RowKey can not be null"),
            };

            let content = serde_json::to_vec(&err_model).unwrap();

            HttpOutput::Content {
                headers: WebContentType::Json.into(),
                status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                content,
            }
            .into_http_fail_result(true, true)
        }
        DbEntityParseFail::FieldTimeStampIsRequired {
            partition_key,
            row_key,
        } => {
            let err_model = OperationFailHttpContract {
                reason: OperationFailReason::RequiredEntityFieldIsMissing,
                message: format!(
                    "Entity with PartitionKey '{}' RowKey '{}' does not contain TimeStamp",
                    partition_key, row_key
                ),
            };

            let content = serde_json::to_vec(&err_model).unwrap();

            HttpOutput::Content {
                headers: WebContentType::Json.into(),
                status_code: OPERATION_FAIL_HTTP_STATUS_CODE,
                content,
            }
            .into_http_fail_result(true, true)
        }
    }
}
