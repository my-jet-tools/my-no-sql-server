use my_http_server::macros::*;
use my_http_server::*;
use std::sync::Arc;

use crate::app::AppContext;
#[http_route(
    method: "GET",
    route: "/",
)]
pub struct IndexAction {
    pub app: Arc<AppContext>,
}

impl IndexAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &IndexAction,
    _: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let content = format!(
        r###"<html><head><title>{ver} MyNoSQLServer</title>
        <link href="/css/bootstrap.css" rel="stylesheet" type="text/css" />
        <link href="/css/site.css" rel="stylesheet" type="text/css" />
        <script src="/js/jquery.js"></script><script src="/js/app.js?ver={rnd}"></script>
        </head><body></body></html>"###,
        ver = crate::app::APP_VERSION,
        rnd = action.app.process_id
    );

    HttpOutput::as_html(content).into_ok_result(true)
}
