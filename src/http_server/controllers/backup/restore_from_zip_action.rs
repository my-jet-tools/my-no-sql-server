use super::UploadedFile;
use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;
use crate::operations::backup::BackupError;

#[http_route(
    method: "POST",
    route: "/api/Backup/RestoreFromZip",
    description: "Restore database from backup zip file",
    summary: "Restore database from backup zip file",
    controller: "Backup",
    input_data: RestoreFromBackupZipFileInputData,
    result:[
        {status_code: 204, description: "Restored ok"},
    ]
)]
pub struct RestoreFromZipAction {
    app: Arc<AppContext>,
}

impl RestoreFromZipAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &RestoreFromZipAction,
    input_data: RestoreFromBackupZipFileInputData,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace(&action.app, ctx).await?;

    let table_name = input_data.get_table_name().map(|itm| itm.to_string());

    let restore_result = crate::operations::backup::restore(
        &action.app,
        &db_namespace,
        input_data.zip.into_content(),
        table_name.as_deref(),
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
pub struct RestoreFromBackupZipFileInputData {
    #[http_form_data(
        name = "tableName",
        description = "Name of the table or '*' for all tables"
    )]
    pub table_name: String,

    #[http_form_data(name = "fileName", description = "File in backup folder")]
    pub zip: UploadedFile,

    #[http_form_data(name = "cleanTable", description = "Clean table before restore")]
    pub clean_table: bool,
}

impl RestoreFromBackupZipFileInputData {
    pub fn get_table_name(&self) -> Option<&str> {
        if self.table_name == "*" {
            None
        } else {
            Some(self.table_name.as_str())
        }
    }
}
