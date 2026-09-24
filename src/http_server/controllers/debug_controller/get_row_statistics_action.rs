use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

use super::contracts::*;

#[http_route(
    method: "GET",
    route: "/api/Debug/GetRowStatistics",
    deprecated_routes: ["/Debug/GetRowStatistics"],
    summary: "Get DbRow statistics",
    description: "Returns DbRow statistics",
    controller: "Debug",
    input_data: "GetRowStatisticsInputData",
    result:[
        {status_code: 200, description: "Successful operation", model: "RowStatisticsContract"},
    ]
)]
pub struct GetRowStatisticsAction {
    app: Arc<AppContext>,
}

impl GetRowStatisticsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetRowStatisticsAction,
    http_input: GetRowStatisticsInputData,
    ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        http_input.table_name.as_str(),
    )
    .await?;

    let result = {
        let read_access = db_table.data.read();

        let db_partition = read_access.get_partition(&http_input.partition_key);

        if db_partition.is_none() {
            return HttpOutput::as_text("Partition not found".to_string())
                .into_ok_result(true)
                .into();
        }

        let db_partition = db_partition.unwrap();

        let db_row = db_partition.get_row(&http_input.row_key);

        if db_row.is_none() {
            return HttpOutput::as_text("Row not found".to_string())
                .into_ok_result(true)
                .into();
        }

        let db_row = db_row.unwrap();

        RowStatisticsContract {
            partition_read_time: db_partition.last_read_moment.to_rfc3339(),
            partition_write_time: db_partition.last_write_moment.to_rfc3339(),
            row_read_time: db_row.get_last_read_access().to_rfc3339(),
            row_write_time: db_row.get_time_stamp_as_date_time().to_rfc3339(),
        }
    };

    HttpOutput::as_json(result).into_ok_result(true).into()
}
