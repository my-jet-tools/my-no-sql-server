use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

use super::models::RowsCountInputContract;
#[http_route(
    method: "GET",
    route: "/api/Count",
    deprecated_routes: ["/Count"],
    controller: "Row",
    description: "Get Rows Count",
    summary: "Returns Rows Count",
    input_data: "RowsCountInputContract",
    result:[
        {status_code: 200, description: "Amount of rows of the table or the partition"},
    ]
)]
pub struct RowCountAction {
    app: Arc<AppContext>,
}

impl RowCountAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

/*
#[async_trait::async_trait]
impl GetAction for RowCountAction {
    fn get_route(&self) -> &str {
        "/Count"
    }

    fn get_description(&self) -> Option<HttpActionDescription> {
        HttpActionDescription {
            controller_name: super::consts::CONTROLLER_NAME,
            description: "Get Rows Count",

            input_params: RowsCountInputContract::get_input_params().into(),
            results: vec![HttpResult {
                http_code: 200,
                nullable: false,
                description: "Amount of rows of the table or the partition".to_string(),
                data_type: HttpDataType::as_long(),
            }],
        }
        .into()
    }

    async fn handle_request(&self, ctx: &mut HttpContext) -> Result<HttpOkResult, HttpFailResult> {
        let input_data = RowsCountInputContract::parse_http_input(ctx).await?;

        let db_table = crate::db_operations::read::table::get(
            self.app.as_ref(), &db_namespace,
            input_data.table_name.as_str(),
        )
        .await?;

        if let Some(partition_key) = input_data.partition_key {
            let table_access = db_table.data.read();

            let partition = table_access.get_partition(partition_key.as_str());

            if let Some(partition) = partition {
                return HttpOutput::as_text(partition.rows_count().to_string())
                    .into_ok_result(true)
                    .into();
            } else {
                return HttpOutput::as_text("0".to_string())
                    .into_ok_result(true)
                    .into();
            }
        }

        let table_access = db_table.data.read();

        let mut result = 0;

        for partition in table_access.get_partitions() {
            result += partition.rows_count();
        }

        return HttpOutput::as_text(result.to_string())
            .into_ok_result(true)
            .into();
    }
}
 */

async fn handle_request(
    action: &RowCountAction,
    input_data: RowsCountInputContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_str(),
    )
    .await?;

    if let Some(partition_key) = input_data.partition_key {
        let table_access = db_table.data.read();

        let partition = table_access.get_partition(partition_key.as_str());

        if let Some(partition) = partition {
            return HttpOutput::as_text(partition.rows_count().to_string())
                .into_ok_result(true)
                .into();
        } else {
            return HttpOutput::as_text("0".to_string())
                .into_ok_result(true)
                .into();
        }
    }

    let table_access = db_table.data.read();

    let mut result = 0;

    for partition in table_access.get_partitions() {
        result += partition.rows_count();
    }

    return HttpOutput::as_text(result.to_string())
        .into_ok_result(true)
        .into();
}
