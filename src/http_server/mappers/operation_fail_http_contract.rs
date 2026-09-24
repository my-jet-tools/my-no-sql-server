use my_http_server::macros::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, MyHttpIntegerEnum)]
pub enum OperationFailReason {
    #[http_enum_case(id = -1; description = "Table already exists")]
    TableAlreadyExists,
    #[http_enum_case(id = -2; description = "Table not found")]
    TableNotFound,
    #[http_enum_case(id = -3; description = "Record already exists")]
    RecordAlreadyExists,
    #[http_enum_case(id = -4; description = "Entity required field is missing")]
    RequiredEntityFieldIsMissing,
    #[http_enum_case(id = -5; description = "Invalid json")]
    JsonParseFail,
    #[http_enum_case(id = -6; description = "Namespace not found")]
    NamespaceNotFound,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct OperationFailHttpContract {
    pub reason: OperationFailReason,
    pub message: String,
}
