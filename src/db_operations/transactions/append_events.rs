use crate::{app::AppContext, db_transactions::steps::TransactionalOperationStep};

use super::TransactionOperationError;

pub async fn append_events(
    app: &AppContext,
    transaction_id: &str,
    transactions: Vec<TransactionalOperationStep>,
) -> Result<(), TransactionOperationError> {
    let result = app
        .active_transactions
        .add_events(transaction_id, transactions)
        .await;

    if !result {
        return Err(TransactionOperationError::TransactionNotFound {
            id: transaction_id.to_string(),
        });
    }

    Ok(())
}
