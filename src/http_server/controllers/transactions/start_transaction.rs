use super::models::StartTransactionResponse;
use crate::app::AppContext;
use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use std::sync::Arc;

#[http_route(
    method: "POST",
    route: "/api/Transactions/Start",
    deprecated_routes: ["/Transactions/Start"],
    description: "Start new Transaction",
    summary: "Starts new Transaction",
    controller: "Transactions",
    result:[
        {status_code: 200, description: "Issued transaction", model: "StartTransactionResponse"},        
    ]
)]

pub struct StartTransactionAction {
    app: Arc<AppContext>,
}

impl StartTransactionAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

/*

#[async_trait]
impl PostAction for StartTransactionAction {
    fn get_route(&self) -> &str {
        "/Transactions/Start"
    }

    fn get_description(&self) -> Option<HttpActionDescription> {
        HttpActionDescription {
            controller_name: super::consts::CONTROLLER_NAME,
            description: "Start new Transaction",

            input_params: None,
            results: vec![HttpResult {
                http_code: 200,
                nullable: true,
                description: "Issued transaction".to_string(),
                data_type: StartTransactionResponse::get_http_data_structure()
                    .into_http_data_type_object(),
            }],
        }
        .into()
    }
}
 */

async fn handle_request(
    action: &StartTransactionAction,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace(&action.app, ctx).await?;

    let transaction_id = action
        .app
        .active_transactions
        .issue_new(db_namespace.name.clone())
        .await;

    let response = StartTransactionResponse { transaction_id };

    HttpOutput::as_json(response).into_ok_result(true).into()
}
