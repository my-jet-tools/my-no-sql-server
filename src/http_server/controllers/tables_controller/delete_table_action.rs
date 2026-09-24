use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::{app::AppContext, db_sync::EventSource};

use super::{super::super::contracts::input_params::*, models::DeleteTableContract};

#[http_route(
    method: "DELETE",
    route: "/api/Tables/Delete",
    deprecated_routes: ["/Tables/Delete"],
    input_data: "DeleteTableContract",
    description: "Delete Table",
    summary: "Deletes Table",
    controller: "Tables",
    result:[
        {status_code: 202, description: "Table is deleted"},
    ]
)]
pub struct DeleteTableAction {
    app: Arc<AppContext>,
}

impl DeleteTableAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &DeleteTableAction,
    input_data: DeleteTableContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    // The api key is checked BEFORE the namespace is resolved: resolving it
    // creates the namespace, so doing it first would let an unauthorized caller
    // leave a folder on disk with every rejected request.
    if input_data.api_key != action.app.settings.table_api_key.as_str() {
        return Err(HttpFailResult::as_unauthorized(None));
    }

    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let event_src = EventSource::as_client_request(action.app.as_ref());

    crate::db_operations::write::table::delete(
        action.app.clone(),
        db_namespace.clone(),
        input_data.table_name,
        event_src,
        DEFAULT_SYNC_PERIOD.get_sync_moment(),
    )
    .await?;

    return HttpOutput::Empty.into_ok_result(true);
}
