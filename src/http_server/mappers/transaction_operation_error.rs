use my_http_server::{HttpFailResult, HttpOutput, WebContentType};

use crate::db_operations::transactions::TransactionOperationError;

impl From<TransactionOperationError> for HttpFailResult {
    fn from(src: TransactionOperationError) -> Self {
        match src {
            TransactionOperationError::TransactionNotFound { id } => HttpOutput::Content {
                content: format!("Transaction {} not found", id).into_bytes(),
                headers: WebContentType::Text.into(),
                status_code: 401,
            }
            .into_http_fail_result(true, true),
            TransactionOperationError::DbEntityParseFail(err) => {
                super::db_operation_error::from_db_entity_parse_fail_to_http_result(err)
            }
            TransactionOperationError::DbOperationError(op_err) => op_err.into(),
            TransactionOperationError::SerdeJsonError(err) => HttpOutput::Content {
                content: format!("{}", err).into_bytes(),
                headers: WebContentType::Text.into(),
                status_code: 500,
            }
            .into_http_fail_result(true, true),

            TransactionOperationError::JsonParseError(err) => HttpOutput::Content {
                content: format!("{:?}", err).into_bytes(),
                headers: WebContentType::Text.into(),
                status_code: 500,
            }
            .into_http_fail_result(true, true),
        }
    }
}
