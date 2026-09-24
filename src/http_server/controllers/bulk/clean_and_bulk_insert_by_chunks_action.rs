use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use my_no_sql_sdk::core::db_json_entity::JsonTimeStamp;
use std::sync::Arc;

use crate::app::AppContext;
use crate::db_operations::bulk_processes::BulkProcessScope;

use super::models::{BulkProcessResponse, CleanAndBulkInsertByChunksInputContract};

#[http_route(
    method: "POST",
    route: "/api/Bulk/CleanAndBulkInsertByChunks",
    input_data: "CleanAndBulkInsertByChunksInputContract",
    summary: "Uploads a chunk of a clean-and-bulk-insert operation",
    description: "Accumulates rows aside from the table. Call it without processId to start a new process - the issued processId has to be sent with every following chunk and with the commit. Nothing is applied to the table until CleanAndBulkInsertByChunksCommit is called",
    controller: "Bulk",
    result:[
        {status_code: 200, description: "Chunk is accepted", model: "BulkProcessResponse"},
        {status_code: 400, description: "Table not found"},
        {status_code: 404, description: "Process not found - it expired, was committed or the server was restarted"},
        {status_code: 409, description: "Process belongs to another writer session or to another table"},
    ]
)]
pub struct CleanAndBulkInsertByChunksAction {
    app: Arc<AppContext>,
}

impl CleanAndBulkInsertByChunksAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &CleanAndBulkInsertByChunksAction,
    input_data: CleanAndBulkInsertByChunksInputContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace(&action.app, ctx).await?;

    let now = JsonTimeStamp::now();

    let rows_by_partition =
        crate::db_operations::parse_json_entity::parse_grouped_by_partition_key_with_use_timestamp(
            input_data.body.as_slice(),
            input_data.use_timestamp,
            &now,
        )?;

    let session = input_data.session_id.as_deref().filter(|s| !s.is_empty());

    let process_id = match input_data.process_id {
        Some(process_id) => {
            action
                .app
                .active_bulk_processes
                .append(
                    process_id.as_str(),
                    session,
                    input_data.table_name.as_str(),
                    rows_by_partition,
                    now.date_time,
                )
                .await?;

            process_id
        }
        None => {
            // Fail fast on a missing table instead of letting the client upload
            // the whole snapshot and only then find out at the commit.
            crate::db_operations::read::table::get(
                action.app.as_ref(),
                &db_namespace,
                input_data.table_name.as_str(),
            )
            .await?;

            let scope = match input_data.partition_key {
                Some(partition_key) => BulkProcessScope::Partition(partition_key),
                None => BulkProcessScope::Table,
            };

            action
                .app
                .active_bulk_processes
                .start(
                    db_namespace.name.clone(),
                    session,
                    input_data.table_name.as_str(),
                    scope,
                    rows_by_partition,
                    now.date_time,
                )
                .await
        }
    };

    HttpOutput::as_json(BulkProcessResponse { process_id })
        .into_ok_result(false)
        .into()
}
