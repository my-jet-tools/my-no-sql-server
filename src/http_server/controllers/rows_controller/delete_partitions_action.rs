use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use std::sync::Arc;

use crate::{app::AppContext, db_sync::EventSource};

use super::models::DeletePartitionsInputContract;

#[http_route(
    method: "DELETE",
    route: "/api/Rows/DeletePartitions",
    deprecated_routes: ["/Rows/DeletePartitions"],
    controller: "Rows",
    description: "Delete Partitions",
    summary: "Deletes Partitions",
    input_data: "DeletePartitionsInputContract",
    result:[
        {status_code: 200, description: "Removed entities"},
    ]
)]
pub struct DeletePartitionsAction {
    app: Arc<AppContext>,
}

impl DeletePartitionsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &DeletePartitionsAction,
    input_data: DeletePartitionsInputContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_ref(),
    )
    .await?;

    let event_src = EventSource::as_client_request(action.app.as_ref());

    let now = DateTimeAsMicroseconds::now();

    crate::db_operations::write::delete_partitions(
        action.app.as_ref(),
        &db_namespace,
        &db_table,
        input_data.partition_keys.into_iter(),
        event_src,
        input_data.sync_period.get_sync_moment(),
        now,
    )
    .await?;

    HttpOutput::Empty.into_ok_result(true).into()
}
