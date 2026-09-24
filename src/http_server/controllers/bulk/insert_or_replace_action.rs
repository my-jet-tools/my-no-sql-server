use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use my_no_sql_sdk::core::db_json_entity::JsonTimeStamp;
use std::sync::Arc;

use crate::app::AppContext;
use crate::db_sync::EventSource;

use super::models::BulkInsertOrReplaceInputContract;

#[http_route(
    method: "POST",
    route: "/api/Bulk/InsertOrReplace",
    deprecated_routes: ["/Bulk/InsertOrReplace"],
    input_data: "BulkInsertOrReplaceInputContract",

    summary: "Bulk insert or replace operation",
    description: "Executes Bulk insert or replace operation",
    controller: "Bulk",
    result:[
        {status_code: 202, description: "Successful operation"},
        {status_code: 404, description: "Table not found"},
    ]
)]
pub struct BulkInsertOrReplaceAction {
    app: Arc<AppContext>,
}

impl BulkInsertOrReplaceAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &BulkInsertOrReplaceAction,
    input_data: BulkInsertOrReplaceInputContract,
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
        crate::db_operations::parse_json_entity::parse_grouped_by_partition_key_with_use_timestamp(
            input_data.body.as_slice(),
            input_data.use_timestamp,
            &now,
        )?;

    crate::db_operations::write::bulk_insert_or_update::execute(
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
