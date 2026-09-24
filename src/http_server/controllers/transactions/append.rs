use crate::app::AppContext;
use my_http_server::macros::*;
use std::sync::Arc;

use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};

use super::models::ProcessTransactionInputModel;

#[http_route(
    method: "POST",
    route: "/api/Transactions/Append",
    deprecated_routes: ["/Transactions/Append"],
    description: "Get Table size",
    summary: "Returns Table size",
    input_data: "ProcessTransactionInputModel",
    controller: "Transactions",
    result:[
        {status_code: 202, description: "Actions are added successfully"},        
    ]
)]
pub struct AppendTransactionAction {
    app: Arc<AppContext>,
}

impl AppendTransactionAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &AppendTransactionAction,
    input_model: ProcessTransactionInputModel,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let transactions =
        crate::db_transactions::json_parser::parse_transactions(input_model.body.as_slice())?;

    crate::db_operations::transactions::append_events(
        action.app.as_ref(),
        input_model.transaction_id.as_str(),
        transactions,
    )
    .await?;

    HttpOutput::Empty.into_ok_result(true)
}
