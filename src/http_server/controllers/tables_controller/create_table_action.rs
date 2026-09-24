use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::{app::AppContext, db_sync::EventSource};

use super::models::CreateTableContract;

#[http_route(
    method: "POST",
    route: "/api/Tables/Create",
    deprecated_routes: ["/Tables/Create"],
    input_data: "CreateTableContract",
    description: "Create table",
    summary: "Creates table",
    controller: "Tables",
    result:[
        {status_code: 202, description: "Table is created"},
        {status_code: 400, description: "Table already exists"},
    ]
)]
pub struct CreateTableAction {
    app: Arc<AppContext>,
}

impl CreateTableAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &CreateTableAction,
    input_data: CreateTableContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace(&action.app, ctx).await?;

    let even_src = EventSource::as_client_request(action.app.as_ref());

    crate::db_operations::write::table::create(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_str(),
        input_data.persist,
        input_data.max_partitions_amount,
        input_data.max_rows_per_partition_amount,
        input_data.compressed,
        even_src,
        input_data.sync_period.get_sync_moment(),
    )
    .await?;

    return HttpOutput::Empty.into_ok_result(true);
}
