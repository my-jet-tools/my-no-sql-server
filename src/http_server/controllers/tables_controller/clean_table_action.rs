use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::{app::AppContext, db_sync::EventSource};

use super::models::CleanTableContract;

#[http_route(
    method: "PUT",
    route: "/api/Tables/Clean",
    deprecated_routes: ["/Tables/Clean"],
    input_data: "CleanTableContract",
    description: "Clean Table",
    summary: "Cleans Table",
    controller: "Tables",
    result:[
        {status_code: 202, description: "Table is cleaned"},
    ]
)]
pub struct CleanTableAction {
    app: Arc<AppContext>,
}

impl CleanTableAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
async fn handle_request(
    action: &CleanTableAction,
    input_data: CleanTableContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_ref(),
    )
    .await?;

    let event_src = EventSource::as_client_request(action.app.as_ref());

    crate::db_operations::write::clean_table(
        action.app.as_ref(),
        &db_namespace,
        &db_table,
        event_src,
        input_data.sync_period.get_sync_moment(),
    )
    .await?;

    return HttpOutput::Empty.into_ok_result(true);
}
