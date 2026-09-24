use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use std::sync::Arc;

use crate::db_operations;

use crate::app::AppContext;
use crate::db_sync::EventSource;

use super::models::{BaseDbRowContract, DeleteRowIfInputModel};

#[http_route(
    method: "DELETE",
    route: "/api/Row/DeleteIf",
    controller: "Row",
    description: "Deletes the row only when it is still at the given version",
    summary: "Deletes the row only when the TimeStamp stored in the table is still the one sent here. A row which has been rewritten meanwhile answers 409 and stays in place",
    input_data: "DeleteRowIfInputModel",
    result:[
        {status_code: 200, description: "Deleted row",  model:"BaseDbRowContract"},
        {status_code: 400, description: "Table not found, or timeStamp is not a valid TimeStamp"},
        {status_code: 404, description: "Row not found"},
        {status_code: 409, description: "Row has been changed since it was read"},
    ]
)]
pub struct DeleteRowIfAction {
    app: Arc<AppContext>,
}

impl DeleteRowIfAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &DeleteRowIfAction,
    http_input: DeleteRowIfInputModel,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        http_input.table_name.as_ref(),
    )
    .await?;

    // A version which can not be read can never match a stored one, so it is refused
    // as a bad request instead of coming back as a concurrency conflict.
    let Some(expected_time_stamp) =
        my_no_sql_sdk::abstractions::parse_time_stamp(http_input.time_stamp.as_str())
    else {
        return Err(HttpFailResult::as_validation_error(format!(
            "'{}' is not a valid TimeStamp",
            http_input.time_stamp
        )));
    };

    let event_src = EventSource::as_client_request(action.app.as_ref());

    let now = DateTimeAsMicroseconds::now();

    db_operations::write::delete_row_if::execute(
        action.app.as_ref(),
        &db_namespace,
        &db_table,
        http_input.partition_key,
        http_input.row_key,
        expected_time_stamp,
        event_src,
        http_input.sync_period.get_sync_moment(),
        now,
    )
    .await?
    .into()
}
