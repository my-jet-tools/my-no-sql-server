use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

#[http_route(
    method: "POST",
    route: "/api/Backup/MakeBackup",
    description: "Force creating a snapshot (backup) right now, ignoring the scheduled interval. Snapshots the namespace named by the 'ns' header, or every namespace when none is named",
    summary: "Force creating a snapshot now",
    controller: "Backup",
    result:[
        {status_code: 204, description: "Snapshot created"},
        {status_code: 400, description: "The requested namespace does not exist"},
        {status_code: 500, description: "The backup did not go through for at least one namespace"},
    ]
)]
pub struct MakeBackupAction {
    app: Arc<AppContext>,
}

impl MakeBackupAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &MakeBackupAction,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    // A named namespace is snapshotted on its own, and one which does not exist
    // is refused: "back up X" answered with a 204 that actually backed up
    // everything except X is the worst possible answer to give about a backup.
    // No `ns` at all still means every namespace — that is what a pre-namespace
    // client asks for.
    let failed = match crate::http_server::get_request_namespace_name(ctx) {
        Some(_) => {
            let db_namespace =
                crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

            crate::operations::backup::save_backup_of_namespace(&action.app, &db_namespace).await
        }
        None => crate::operations::backup::save_backup(&action.app, true).await,
    };

    // The status code has to mean "there is a snapshot now", and only that: the
    // namespaces are backed up one by one, so a run can write a perfectly valid
    // archive for one of them and fail on the next. Naming the failed ones — and
    // what went wrong for each — keeps the caller from having to check
    // /api/Backup/List to find out what happened.
    if !failed.is_empty() {
        let details: Vec<String> = failed
            .iter()
            .map(|itm| format!("{} ({})", itm.namespace, itm.reason))
            .collect();

        return Err(HttpFailResult::as_fatal_error(format!(
            "Backup did not go through for namespace(s): {}",
            details.join(", ")
        )));
    }

    HttpOutput::Empty.into_ok_result(true).into()
}
