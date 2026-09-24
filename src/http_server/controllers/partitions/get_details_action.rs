use super::models::*;
use crate::app::AppContext;
use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::{result::Result, sync::Arc};

#[http_route(
    method: "GET",
    route: "/api/Partitions/Details",
    input_data: "GetPartitionsAmountContract",
    description: "Get per-partition metrics (records count and data size) of selected table",
    summary: "Returns records count and data size in bytes for each partition of the table",
    controller: "Partitions",
    result:[
        {status_code: 200, description: "Per-partition metrics"},
        {status_code: 400, description: "Table not found"},
    ]
)]
pub struct GetPartitionsDetailsAction {
    app: Arc<AppContext>,
}

impl GetPartitionsDetailsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetPartitionsDetailsAction,
    input_data: GetPartitionsAmountContract,
    ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_str(),
    )
    .await?;

    let metrics =
        crate::db_operations::read::partitions::get_partitions_metrics(&action.app, &db_table)
            .await?;

    let result: Vec<PartitionMetricHttpModel> = metrics
        .into_iter()
        .map(|itm| PartitionMetricHttpModel {
            partition_key: itm.partition_key,
            records_count: itm.records_count,
            data_size: itm.data_size,
        })
        .collect();

    HttpOutput::as_json(result).into_ok_result(true).into()
}
