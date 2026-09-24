use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;
use crate::operations::backup::BackupError;

#[http_route(
    method: "POST",
    route: "/api/Backup/RestoreFromBackup",
    description: "Restore database from backup folder",
    summary: "Restore database from backup folder",
    controller: "Backup",
    input_data: RestoreFromBackupInputData,
    result:[
        {status_code: 204, description: "Restored ok"},
    ]
)]
pub struct RestoreFromBackupAction {
    app: Arc<AppContext>,
}

impl RestoreFromBackupAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &RestoreFromBackupAction,
    input_data: RestoreFromBackupInputData,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace(&action.app, ctx).await?;

    // Resolving the path here by hand used to read straight out of the backup
    // root, which since namespaces is nobody's folder — and let a file name with
    // a separator in it reach another namespace's snapshots. `restore_from_file`
    // is the one place that resolves a snapshot inside the caller's own
    // namespace and refuses anything that tries to leave it.
    let restore_result = crate::operations::backup::restore_from_file(
        &action.app,
        &db_namespace,
        input_data.file_name.as_str(),
        input_data.get_table_name(),
        input_data.clean_table,
    )
    .await;

    match restore_result {
        Ok(_) => HttpOutput::Empty.into_ok_result(true).into(),
        // A write the database refused answers as itself: a restore which lands
        // before the server has loaded its tables is a 503, and it used to be a
        // panic in the handler.
        Err(BackupError::DbOperation(err)) => Err(err.into()),
        Err(err) => Err(HttpFailResult::as_fatal_error(err.into_message())),
    }
}

#[derive(MyHttpInput)]
pub struct RestoreFromBackupInputData {
    #[http_form_data(
        name = "tableName",
        description = "Name of the table or '*' for all tables"
    )]
    pub table_name: String,

    #[http_form_data(name = "fileName", description = "File in backup folder")]
    pub file_name: String,

    #[http_form_data(name = "cleanTable", description = "Clean table before restore")]
    pub clean_table: bool,
}

impl RestoreFromBackupInputData {
    pub fn get_table_name(&self) -> Option<&str> {
        if self.table_name == "*" {
            None
        } else {
            Some(self.table_name.as_str())
        }
    }
}
