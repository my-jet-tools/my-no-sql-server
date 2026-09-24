use crate::{app::AppContext, http_server::http_sessions::*};
use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use super::models::SubscribeToTableInputModel;

#[http_route(
    method: "POST",
    route: "/api/DataReader/Subscribe",
    deprecated_routes: ["/DataReader/Subscribe"],
    controller: "DataReader",
    summary: "Subscribes to table",
    description: "Subscribe to table",
    input_data: "SubscribeToTableInputModel",
    result:[
        {status_code: 202, description: "Successful operation"},
    ]
)]
pub struct SubscribeAction {
    app: Arc<AppContext>,
}

impl SubscribeAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &SubscribeAction,
    input_data: SubscribeToTableInputModel,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace =
        crate::http_server::get_request_namespace_of_subscribe(&action.app, ctx).await?;

    let data_reader = action
        .app
        .get_http_session(input_data.session_id.as_str())
        .await?;

    let db_namespace = match db_namespace {
        Some(db_namespace) => db_namespace,
        None => {
            // A namespace nobody has written to yet is not a reason to refuse a
            // reader — it gets the same empty snapshot a missing table gets. The
            // name it is sent under is the envelope's only, so the reader's own
            // is as good as the requested one there.
            crate::operations::data_readers::send_empty_snapshot(
                &action.app,
                data_reader.get_namespace(),
                data_reader,
                input_data.table_name.as_str(),
            );

            return HttpOutput::Empty.into_ok_result(true).into();
        }
    };

    crate::operations::data_readers::subscribe(
        &action.app,
        &db_namespace,
        data_reader,
        input_data.table_name.as_str(),
    )
    .await?;

    HttpOutput::Empty.into_ok_result(true).into()
}
