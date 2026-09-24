use crate::{app::AppContext, db_sync::EventSource};
use my_http_server::macros::*;
use std::{result::Result, sync::Arc};

use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};

use super::models::{CreateTableContract, TableContract};

#[http_route(
    method: "POST",
    route: "/api/Tables/CreateIfNotExists",
    deprecated_routes: ["/Tables/CreateIfNotExists"],
    input_data: CreateTableContract,
    description: "Create table if not exists",
    summary: "Creates table if not exists",
    controller: "Tables",
    result:[
        {status_code: 200, description: "Table is created", model: TableContract},
    ]
)]
pub struct CreateIfNotExistsAction {
    app: Arc<AppContext>,
}

impl CreateIfNotExistsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &CreateIfNotExistsAction,
    input_data: CreateTableContract,
    ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace(&action.app, ctx).await?;

    let even_src = EventSource::as_client_request(action.app.as_ref());

    let table = crate::db_operations::write::table::create_if_not_exist(
        &action.app,
        &db_namespace,
        input_data.table_name.as_str(),
        input_data.persist,
        input_data.max_partitions_amount,
        input_data.max_rows_per_partition_amount,
        even_src,
        input_data.sync_period.get_sync_moment(),
    )
    .await?;

    let response = TableContract::from_table_wrapper(&table);

    HttpOutput::as_json(response).into_ok_result(true).into()
}
