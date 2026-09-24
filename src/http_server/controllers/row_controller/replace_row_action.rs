use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult};
use std::sync::Arc;

use my_no_sql_sdk::core::db_json_entity::JsonTimeStamp;

use crate::app::AppContext;
use crate::db_sync::EventSource;
use crate::operations::parse_db_json_entity_to_validate;

use super::models::{BaseDbRowContract, ReplaceInputContract};

#[http_route(
    method: "PUT",
    route: "/api/Row/Replace",
    deprecated_routes: ["/Row/Replace"],
    controller: "Row",
    description: "Replace Entity",
    summary: "Replaces Entity",
    input_data: "ReplaceInputContract",
    result:[
        {status_code: 200, description: "Replaced row",  model:"BaseDbRowContract"},
    ]
)]
pub struct ReplaceRowAction {
    app: Arc<AppContext>,
}

impl ReplaceRowAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &ReplaceRowAction,
    input_data: ReplaceInputContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_ref(),
    )
    .await?;

    let now = JsonTimeStamp::now();
    let db_entity = parse_db_json_entity_to_validate(input_data.body.as_slice(), &now)?;

    let candidate = crate::db_operations::write::replace::validate_before(
        action.app.as_ref(),
        &db_table,
        db_entity,
    )
    .await?;

    let expected_time_stamp = candidate.expected_time_stamp;
    let db_row = Arc::new(candidate.db_row);

    let event_src = EventSource::as_client_request(action.app.as_ref());

    crate::db_operations::write::replace::execute(
        action.app.as_ref(),
        &db_namespace,
        &db_table,
        db_row,
        expected_time_stamp,
        event_src,
        input_data.sync_period.get_sync_moment(),
        &now,
    )
    .await?
    .into()
}
