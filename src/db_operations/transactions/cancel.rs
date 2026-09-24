use crate::app::AppContext;

use super::TransactionOperationError;

pub async fn cancel(
    app: &AppContext,
    transaction_id: &str,
) -> Result<(), TransactionOperationError> {
    let result = app.active_transactions.remove(transaction_id).await;

    match result {
        Some(_) => Ok(()),
        None => Err(TransactionOperationError::TransactionNotFound {
            id: transaction_id.to_string(),
        }),
    }
}
