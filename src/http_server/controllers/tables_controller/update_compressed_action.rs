use super::models::UpdateCompressedTableContract;
use crate::{app::AppContext, db_sync::EventSource};
use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::{result::Result, sync::Arc};

#[http_route(
    method: "POST",
    route: "/api/Tables/UpdateCompressed",
    deprecated_routes: ["/Tables/UpdateCompressed"],

    input_data: "UpdateCompressedTableContract",
    description: "Update table in-memory compression state",
    summary: "Compresses or decompresses the already stored rows of the table",
    controller: "Tables",
    result:[
        {status_code: 202, description: "Updated succesfully"},
        {status_code: 400, description: "Table not found"},
    ]
)]
pub struct UpdateCompressedAction {
    app: Arc<AppContext>,
}

impl UpdateCompressedAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &UpdateCompressedAction,
    input_data: UpdateCompressedTableContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_str(),
    )
    .await?;

    let event_src = EventSource::as_client_request(action.app.as_ref());

    crate::db_operations::write::table::update_compressed_state(
        &action.app,
        &db_namespace,
        db_table,
        input_data.compressed,
        input_data.force_compress,
        event_src,
    )
    .await?;

    HttpOutput::Empty.into_ok_result(true).into()
}
