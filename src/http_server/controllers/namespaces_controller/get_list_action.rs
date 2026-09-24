use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

use super::models::NamespaceContract;

#[http_route(
    method: "GET",
    route: "/api/Namespaces/List",
    controller: "Namespaces",
    description: "Get list of namespaces",
    summary: "Returns every namespace of this server together with the amount of tables in it",
    result:[
        {status_code: 200, description: "List of namespaces", model: "Vec<NamespaceContract>"},
    ]
)]
pub struct GetNamespacesListAction {
    app: Arc<AppContext>,
}

impl GetNamespacesListAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &GetNamespacesListAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    crate::db_operations::check_app_states(action.app.as_ref())?;

    let mut result = Vec::new();

    for db_namespace in action.app.namespaces.get_all() {
        result.push(NamespaceContract {
            name: db_namespace.name.to_string(),
            tables_amount: db_namespace.tables_amount(),
        });
    }

    HttpOutput::as_json(result).into_ok_result(true).into()
}
