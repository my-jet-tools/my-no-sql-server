use my_http_server::{HttpFailResult, HttpOkResult, HttpOutput, WebContentType};

use crate::db_operations::write::WriteOperationResult;

impl Into<Result<HttpOkResult, HttpFailResult>> for WriteOperationResult {
    fn into(self) -> Result<HttpOkResult, HttpFailResult> {
        match self {
            WriteOperationResult::SingleRow(db_row) => {
                let output = HttpOutput::Content {
                    status_code: 200,
                    headers: WebContentType::Json.into(),
                    content: db_row.to_vec(),
                };

                Ok(HttpOkResult {
                    write_telemetry: false,
                    output,
                })
            }
            WriteOperationResult::Empty => HttpOutput::Empty.into_ok_result(true),
        }
    }
}
