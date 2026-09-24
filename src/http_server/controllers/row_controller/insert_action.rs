use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use my_no_sql_sdk::core::db_json_entity::JsonTimeStamp;
use std::sync::Arc;

use crate::app::AppContext;
use crate::db_sync::EventSource;

use super::models::InsertInputContract;

#[http_route(
    method: "POST",
    route: "/api/Row/Insert",
    deprecated_routes: ["/Row/Insert"],
    controller: "Row",
    description: "Insert Row",
    summary: "Inserts Row",
    input_data: "InsertInputContract",
    result:[
        {status_code: 200, description: "Amount of rows of the table or the partition"},
    ]
)]
pub struct InsertRowAction {
    app: Arc<AppContext>,
}

impl InsertRowAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &InsertRowAction,
    input_data: InsertInputContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_str(),
    )
    .await?;

    let now = JsonTimeStamp::now();

    let db_entity =
        crate::operations::parse_db_json_entity_to_validate(input_data.body.as_slice(), &now)?;

    let db_row = crate::db_operations::write::insert::validate_before(
        action.app.as_ref(),
        &db_table,
        db_entity,
    )
    .await?;

    let event_src = EventSource::as_client_request(action.app.as_ref());

    crate::db_operations::write::insert::execute(
        &action.app,
        &db_namespace,
        db_table,
        Arc::new(db_row),
        event_src,
        input_data.sync_period.get_sync_moment(),
        now.date_time,
    )
    .await?;

    HttpOutput::Empty.into_ok_result(true).into()
}
