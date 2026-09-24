use crate::{app::AppContext, db_sync::EventSource};
use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use std::sync::Arc;

use super::models::ProcessTransactionInputModel;

#[http_route(
    method: "POST",
    route: "/api/Transactions/Commit",
    deprecated_routes: ["/Transactions/Commit"],
    description: "Commit transaction",
    summary: "Commits transaction",
    input_data: "ProcessTransactionInputModel",
    controller: "Transactions",
    result:[
        {status_code: 202, description: "Transaction is canceled"},        
    ]
)]
pub struct CommitTransactionAction {
    app: Arc<AppContext>,
}

impl CommitTransactionAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &CommitTransactionAction,
    input_model: ProcessTransactionInputModel,
    _ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let even_src = EventSource::as_client_request(action.app.as_ref());

    let now = DateTimeAsMicroseconds::now();
    crate::db_operations::transactions::commit(
        action.app.as_ref(),
        input_model.transaction_id.as_ref(),
        even_src,
        crate::db_sync::DataSynchronizationPeriod::Sec1.get_sync_moment(),
        now,
    )
    .await?;

    return HttpOutput::Empty.into_ok_result(true).into();
}
