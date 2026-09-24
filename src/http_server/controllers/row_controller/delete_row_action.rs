use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use std::sync::Arc;

use crate::db_operations;

use crate::app::AppContext;
use crate::db_sync::EventSource;

use super::models::{BaseDbRowContract, DeleteRowInputModel};

#[http_route(
    method: "DELETE",
    route: "/api/Row",
    deprecated_routes: ["/Row"],
    controller: "Row",
    description: "Delete Entity",
    summary: "Delete Entity",
    input_data: "DeleteRowInputModel",
    result:[
        {status_code: 200, description: "Deleted row",  model:"BaseDbRowContract"},
    ]
)]
pub struct DeleteRowAction {
    app: Arc<AppContext>,
}

impl DeleteRowAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &DeleteRowAction,
    http_input: DeleteRowInputModel,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        http_input.table_name.as_ref(),
    )
    .await?;

    let event_src = EventSource::as_client_request(action.app.as_ref());

    let now = DateTimeAsMicroseconds::now();

    db_operations::write::delete_row::execute(
        action.app.as_ref(),
        &db_namespace,
        &db_table,
        http_input.partition_key,
        http_input.row_key,
        event_src,
        http_input.sync_period.get_sync_moment(),
        now,
    )
    .await?
    .into()
}
