use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

use crate::app::AppContext;

use super::models::ProcessTransactionInputModel;

#[http_route(
    method: "POST",
    route: "/api/Transactions/Cancel",
    deprecated_routes: ["/Transactions/Cancel"],
    description: "Cancel transaction",
    summary: "Cancels transaction",
    input_data: "ProcessTransactionInputModel",
    controller: "Transactions",
    result:[
        {status_code: 202, description: "Transaction is canceled"},        
    ]
)]
pub struct CancelTransactionAction {
    app: Arc<AppContext>,
}

impl CancelTransactionAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

/*
#[async_trait::async_trait]
impl PostAction for CancelTransactionAction {
    fn get_route(&self) -> &str {
        "/Transactions/Cancel"
    }

    fn get_description(&self) -> Option<HttpActionDescription> {
        HttpActionDescription {
            controller_name: super::consts::CONTROLLER_NAME,
            description: "Cancel transaction",

            input_params: Some(ProcessTransactionInputModel::get_input_params()),
            results: vec![
                response::empty("Transaction is canceled"),
                super::models::transaction_not_found_response_doc(),
            ],
        }
        .into()
    }


}
 */

async fn handle_request(
    action: &CancelTransactionAction,
    input_model: ProcessTransactionInputModel,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    crate::db_operations::transactions::cancel(
        action.app.as_ref(),
        input_model.transaction_id.as_str(),
    )
    .await?;
    return HttpOutput::Empty.into_ok_result(true);
}
