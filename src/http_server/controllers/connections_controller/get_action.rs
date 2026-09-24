use crate::app::AppContext;
use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use super::models::ConnectionsModel;

#[http_route(
    method: "GET",
    route: "/api/Connections",
    controller: "Monitoring",
    description: "Connections traffic metrics",
    summary: "Returns per-reader and aggregated incoming/outgoing bytes per second",
    result:[
        {status_code: 200, description: "Connections traffic snapshot", model: "ConnectionsModel"},
    ]
)]
pub struct GetConnectionsAction {
    app: Arc<AppContext>,
}

impl GetConnectionsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetConnectionsAction,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let model = ConnectionsModel::new(action.app.as_ref()).await;
    HttpOutput::as_json(model).into_ok_result(true).into()
}
