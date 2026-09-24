use my_http_server::{HttpFailResult, HttpOutput, WebContentType};

use crate::db_operations::bulk_processes::BulkProcessError;

impl From<BulkProcessError> for HttpFailResult {
    fn from(src: BulkProcessError) -> Self {
        match src {
            BulkProcessError::ProcessNotFound { id } => HttpOutput::Content {
                content: format!("Bulk process {} not found", id).into_bytes(),
                headers: WebContentType::Text.into(),
                status_code: 404,
            }
            .into_http_fail_result(true, true),
            BulkProcessError::ProcessDoesNotMatch { id, reason } => HttpOutput::Content {
                content: format!("Bulk process {} does not match: {}", id, reason).into_bytes(),
                headers: WebContentType::Text.into(),
                status_code: 409,
            }
            .into_http_fail_result(true, true),
            BulkProcessError::DbOperationError(err) => err.into(),
        }
    }
}
