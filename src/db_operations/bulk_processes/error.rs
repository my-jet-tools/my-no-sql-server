use crate::db_operations::DbOperationError;

#[derive(Debug)]
pub enum BulkProcessError {
    /// The process is unknown to this server: it was never started, it was
    /// already committed/canceled, it expired by GC, or the server restarted
    /// in the middle of the upload. The client has to restart the whole upload.
    ProcessNotFound {
        id: String,
    },

    /// The process id belongs to another writer session or to another table.
    /// Fails loudly instead of silently mixing chunks of two uploads.
    ProcessDoesNotMatch {
        id: String,
        reason: String,
    },

    DbOperationError(DbOperationError),
}

impl From<DbOperationError> for BulkProcessError {
    fn from(src: DbOperationError) -> Self {
        Self::DbOperationError(src)
    }
}
