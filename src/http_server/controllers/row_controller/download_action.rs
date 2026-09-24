use my_http_server::macros::*;
use my_http_server::HttpResponseHeaders;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput, WebContentType};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use std::sync::Arc;

use crate::app::AppContext;
use crate::db_operations::read::ReadOperationResult;
use crate::db_operations::UpdateStatistics;
use crate::http_server::mappers::{try_compress_zstd, wants_zstd, COMPRESSION_THRESHOLD};

use super::models::*;

#[http_route(
    method: "GET",
    route: "/api/Row/Download",
    controller: "Row",
    description: "Download partition rows as a JSON file",
    summary: "Returns rows with Content-Disposition: attachment",
    input_data: "GetRowInputModel",
    result:[
        {status_code: 200, description: "JSON file with rows"},
    ]
)]
pub struct DownloadRowsAction {
    app: Arc<AppContext>,
}

impl DownloadRowsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &DownloadRowsAction,
    input_data: GetRowInputModel,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_ref(),
    )
    .await?;

    let now = DateTimeAsMicroseconds::now();

    let result = if let Some(partition_key) = input_data.partition_key.as_ref() {
        crate::db_operations::read::rows::get_all_by_partition_key(
            &action.app,
            &db_table,
            partition_key,
            input_data.limit,
            input_data.skip,
            no_update_statistics(),
            now,
        )
        .await?
    } else {
        crate::db_operations::read::rows::get_all(
            &action.app,
            &db_table,
            input_data.limit,
            input_data.skip,
            no_update_statistics(),
            now,
        )
        .await?
    };

    let content = match result {
        ReadOperationResult::SingleRow(c) => c,
        ReadOperationResult::RowsArray(c) => c,
        ReadOperationResult::EmptyArray => vec![b'[', b']'],
    };

    let filename = build_filename(
        input_data.table_name.as_ref(),
        input_data.partition_key.as_deref(),
    );

    let mut headers = HttpResponseHeaders::new(Some(WebContentType::Json));
    headers.add_header(
        "Content-Disposition".into(),
        format!("attachment; filename=\"{}\"", filename),
    );

    let response = HttpOkResult {
        write_telemetry: true,
        output: HttpOutput::Content {
            status_code: 200,
            headers,
            content,
        },
    };

    Ok(if wants_zstd(input_data.x_compress.as_deref()) {
        try_compress_zstd(response, COMPRESSION_THRESHOLD)
    } else {
        response
    })
}

fn build_filename(table_name: &str, partition_key: Option<&str>) -> String {
    let sanitized_table = sanitize(table_name);
    match partition_key {
        Some(pk) if !pk.is_empty() => format!("{}_{}.json", sanitized_table, sanitize(pk)),
        _ => format!("{}.json", sanitized_table),
    }
}

fn no_update_statistics() -> UpdateStatistics {
    UpdateStatistics {
        update_partition_last_read_access_time: false,
        update_rows_last_read_access_time: false,
        update_partition_expiration_time: None,
        update_rows_expiration_time: None,
    }
}

fn sanitize(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '_',
        })
        .collect()
}
