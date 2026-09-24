use my_no_sql_sdk::core::db_json_entity::DbEntityParseFail;

#[derive(Debug)]
pub enum DbOperationError {
    TableNotFound(String),
    TableAlreadyExists,
    RecordAlreadyExists,
    TimeStampFieldRequires,
    RecordNotFound,
    OptimisticConcurrencyUpdateFails,
    TableNameValidationError(String),
    NamespaceNameValidationError(String),
    /// Raised by operations which refuse to bring a namespace into existence —
    /// a delete has nothing to delete in a namespace that does not exist, and
    /// creating one just to fail inside it would leave a folder behind.
    NamespaceNotFound(String),
    ApplicationIsNotInitializedYet,
    DbEntityParseFail(DbEntityParseFail),
}

impl DbOperationError {
    pub fn is_app_is_not_initialized(&self) -> bool {
        match self {
            DbOperationError::ApplicationIsNotInitializedYet => true,
            _ => false,
        }
    }
}

impl From<DbEntityParseFail> for DbOperationError {
    fn from(value: DbEntityParseFail) -> Self {
        Self::DbEntityParseFail(value)
    }
}
