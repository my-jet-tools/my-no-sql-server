use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult};
use std::str;
use std::sync::Arc;

use my_no_sql_sdk::core::db_json_entity::JsonTimeStamp;

use crate::app::AppContext;
use crate::db_sync::EventSource;
use crate::operations::parse_db_json_entity_and_keep_date_time;

use super::models::InsertOrReplaceIfNewInputContract;

#[http_route(
    method: "POST",
    route: "/api/Row/InsertOrReplaceIfNew",
    deprecated_routes: ["/Row/InsertOrReplaceIfNew"],
    controller: "Row",
    description: "Insert or replace DbEntity only when it is newer than the stored one",
    summary: "Inserts a missing DbEntity, or replaces it only when the incoming TimeStamp is greater than the stored one",
    input_data: "InsertOrReplaceIfNewInputContract",
    result:[
        {status_code: 200, description: "Removed entities"},
    ]
)]
pub struct InsertOrReplaceIfNewAction {
    app: Arc<AppContext>,
}

impl InsertOrReplaceIfNewAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &InsertOrReplaceIfNewAction,
    input_data: InsertOrReplaceIfNewInputContract,
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

    let db_row = parse_db_json_entity_and_keep_date_time(input_data.body.as_slice())?;

    crate::db_operations::write::insert_or_replace_if_new::execute(
        action.app.as_ref(),
        &db_namespace,
        db_table,
        Arc::new(db_row),
        event_src,
        input_data.sync_period.get_sync_moment(),
        now.date_time,
    )
    .await?
    .into()
}
