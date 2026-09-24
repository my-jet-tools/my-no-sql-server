use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::app::AppContext;
use crate::db_sync::EventSource;

use super::models::BulkDeleteInputContract;

#[http_route(
    method: "POST",
    route: "/api/Bulk/Delete",
    deprecated_routes: ["/Bulk/Delete"],
    input_data: "BulkDeleteInputContract",
    summary: "Bulk delete operation",
    description: "Does bulk delete operation",
    controller: "Bulk",
    result:[
        {status_code: 202, description: "Successful operation"},
        {status_code: 404, description: "Table not found"},
    ]
)]
pub struct BulkDeleteAction {
    app: Arc<AppContext>,
}

impl BulkDeleteAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &BulkDeleteAction,
    input_data: BulkDeleteInputContract,
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

    let rows_to_delete: BTreeMap<String, Vec<String>> =
        serde_json::from_slice(input_data.body.as_slice()).unwrap();

    let now = DateTimeAsMicroseconds::now();

    crate::db_operations::write::bulk_delete(
        action.app.as_ref(),
        &db_namespace,
        db_table.as_ref(),
        rows_to_delete.into_iter(),
        event_src,
        input_data.sync_period.get_sync_moment(),
        now,
    )
    .await?;

    HttpOutput::Empty.into_ok_result(true).into()
}
