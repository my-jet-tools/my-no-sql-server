use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput, WebContentType};
use std::sync::Arc;

use crate::{
    app::AppContext,
    data_readers::{http_connection::HttpPayload, DataReaderConnection},
    db_operations::DbOperationError,
    http_server::{controllers::mappers::ToSetExpirationTime, http_sessions::HttpSessionsSupport},
};

use super::models::{GetChangesInputModel, UpdateExpirationDateTime};
#[http_route(
    method: "POST",
    route: "/api/DataReader/GetChanges",
    deprecated_routes: ["/DataReader/GetChanges"],
    controller: "DataReader",
    description: "Get Subscriber changes",
    summary: "Returns Subscriber changes",
    input_data: "GetChangesInputModel",
    result:[
        {status_code: 200, description: "Successful operation"},
    ]
)]
pub struct GetChangesAction {
    app: Arc<AppContext>,
}

impl GetChangesAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetChangesAction,
    input_data: GetChangesInputModel,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let data_reader = action
        .app
        .get_http_session(input_data.session_id.as_str())
        .await?;

    let body_model = input_data.body.deserialize_json()?;

    for update_model in &body_model.update_expiration_time {
        update_expiration_time(
            &action.app,
            &db_namespace,
            update_model.table_name.as_str(),
            &update_model.items,
        )
        .await?;
    }

    if let DataReaderConnection::Http(info) = &data_reader.connection {
        let result = info.new_request().await?;
        match result {
            HttpPayload::Ping => return HttpOutput::Empty.into_ok_result(false).into(),
            HttpPayload::Payload(payload) => {
                return HttpOutput::Content {
                    status_code: 200,
                    headers: Default::default(),
                    content: payload,
                }
                .into_ok_result(false)
                .into();
            }
        }
    }

    HttpOutput::Content {
        status_code: 400,
        content: "Only HTTP sessions are supported".to_string().into_bytes(),
        headers: WebContentType::Text.into(),
    }
    .into_err(true, true)
}

async fn update_expiration_time(
    _app: &Arc<AppContext>,
    db_namespace: &Arc<crate::app::DbNamespace>,
    table_name: &str,
    items: &[UpdateExpirationDateTime],
) -> Result<(), DbOperationError> {
    let db_table = db_namespace.db.get_table(table_name);
    if db_table.is_none() {
        return Ok(());
    }

    let db_table = db_table.unwrap();

    for item in items {
        if let Some(set_expiration_time) = item
            .set_db_partition_expiration_time
            .to_set_expiration_time()
        {
            crate::db_operations::update_partition_expiration_time(
                &db_table,
                item.partition_key.to_string(),
                set_expiration_time,
            )
        }

        if let Some(set_expiration_time) = item.set_db_rows_expiration_time.to_set_expiration_time()
        {
            crate::db_operations::update_rows_expiration_time(
                db_namespace,
                &db_table,
                &item.partition_key,
                item.row_keys.iter().map(|x| x.as_str()),
                set_expiration_time,
            );
        }
    }

    Ok(())
}
