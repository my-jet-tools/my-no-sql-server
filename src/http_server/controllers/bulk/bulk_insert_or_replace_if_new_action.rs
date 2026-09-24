use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use my_no_sql_sdk::core::db_json_entity::JsonTimeStamp;
use std::sync::Arc;

use crate::app::AppContext;
use crate::db_sync::EventSource;

use super::models::BulkInsertOrReplaceIfNewInputContract;

#[http_route(
    method: "POST",
    route: "/api/Bulk/InsertOrReplaceIfNew",
    deprecated_routes: ["/Bulk/InsertOrReplaceIfNew"],
    input_data: "BulkInsertOrReplaceIfNewInputContract",

    summary: "Bulk insert or replace if the row is newer",
    description: "Bulk operation that inserts a row when it is missing, or replaces it only when the incoming TimeStamp is greater than the stored one",
    controller: "Bulk",
    result:[
        {status_code: 202, description: "Successful operation"},
        {status_code: 404, description: "Table not found"},
    ]
)]
pub struct BulkInsertOrReplaceIfNewAction {
    app: Arc<AppContext>,
}

impl BulkInsertOrReplaceIfNewAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &BulkInsertOrReplaceIfNewAction,
    input_data: BulkInsertOrReplaceIfNewInputContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_str(),
    )
    .await?;

    let event_src = EventSource::as_client_request(action.app.as_ref());

    let now = JsonTimeStamp::now();

    let rows_by_partition =
        crate::db_operations::parse_json_entity::parse_grouped_by_partition_key_and_keep_date_time(
            input_data.body.as_slice(),
        )?;

    crate::db_operations::write::bulk_insert_or_replace_if_new::execute(
        action.app.as_ref(),
        &db_namespace,
        &db_table,
        rows_by_partition,
        event_src,
        input_data.sync_period.get_sync_moment(),
        now.date_time,
    )
    .await?;

    HttpOutput::Empty.into_ok_result(true).into()
}
