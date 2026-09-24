use std::sync::Arc;

use my_http_server::{
    macros::{http_route, MyHttpInput, MyHttpObjectStructure},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};
use my_no_sql_sdk::server::rust_extensions::date_time::DateTimeAsMicroseconds;
use serde::{Deserialize, Serialize};

use crate::app::AppContext;

#[http_route(
    method: "POST",
    route: "/api/Ping",
    controller: "Monitoring",
    description: "Endpoint to ping the service",
    summary: "Endpoint to ping the service",
    input_data: PingHttpInputModel,
    result:[
        {status_code: 200, description: "Issued/refreshed writer session", model: "PingResult"},
    ]
)]
pub struct PingAction {
    app: Arc<AppContext>,
}

impl PingAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &PingAction,
    input_data: PingHttpInputModel,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let now = DateTimeAsMicroseconds::now();

    let connection_addr = ctx.request.addr.to_string();

    let session = input_data.session_id.as_deref().filter(|s| !s.is_empty());

    let session = action
        .app
        .http_writers
        .get_or_create(
            session,
            crate::http_server::get_namespace_header(ctx)
                .unwrap_or(my_no_sql_sdk::DEFAULT_NAMESPACE),
            &input_data.name,
            &input_data.version,
            input_data.tables.iter().map(|itm| itm.as_str()),
            connection_addr,
            now,
        )
        .await;

    HttpOutput::as_json(PingResult { session })
        .into_ok_result(false)
        .into()
}

#[derive(Debug, MyHttpInput)]
pub struct PingHttpInputModel {
    #[http_body(name = "name", description = "Client Name")]
    pub name: String,
    #[http_body(name = "version", description = "Client Version")]
    pub version: String,

    #[http_body(name = "tables", description = "List of tables with")]
    pub tables: Vec<String>,

    #[http_header(
        name = "session",
        description = "Writer session id issued by a previous ping"
    )]
    pub session_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct PingResult {
    #[serde(rename = "session")]
    pub session: String,
}
