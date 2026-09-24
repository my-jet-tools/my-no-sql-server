use my_http_server::{macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;

use super::models::IsAliveResponse;

#[http_route(
    method: "GET",
    route: "/api/IsAlive",
    controller: "Monitoring",
    description: "Returns model shows that service is alive",
    summary: "Returns model shows that service is alive",
    result:[
        {status_code: 200, description: "Monitoring result", model: "IsAliveResponse"},
    ]
)]
pub struct IsAliveAction;

async fn handle_request(
    _: &IsAliveAction,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let version = env!("CARGO_PKG_VERSION");

    let env_info = match std::env::var("ENV_INFO") {
        Ok(value) => Some(value),
        Err(_) => None,
    };

    let time = DateTimeAsMicroseconds::now();

    let response = IsAliveResponse {
        name: "MyNoSqlServer".to_string(),
        time: time.to_rfc3339(),
        version: version.to_string(),
        env_info,
    };

    HttpOutput::as_json(response).into_ok_result(false).into()
}
