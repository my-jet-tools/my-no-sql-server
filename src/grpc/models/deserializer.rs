use my_no_sql_sdk::core::db_json_entity::JsonTimeStamp;

use crate::{
    db_transactions::steps::{TransactionalOperationStep, UpdateRowsStepState},
    mynosqlserver_grpc::*,
};

use super::{
    CleanTableTransactionActionGrpcModel, DeletePartitionsTransactionActionGrpcModel,
    DeleteRowsTransactionActionGrpcModel, GrpcContractConvertError,
    InsertOrReplaceEntitiesTransactionActionGrpcModel,
};

pub fn deserialize(
    transaction_type: TransactionType,
    content: &[u8],
    now: &JsonTimeStamp,
) -> Result<TransactionalOperationStep, GrpcContractConvertError> {
    match transaction_type {
        TransactionType::CleanTable => {
            let contract = CleanTableTransactionActionGrpcModel::deserialize(content)?;

            let result = TransactionalOperationStep::CleanTable {
                table_name: contract.table_name,
            };
            Ok(result)
        }
        TransactionType::DeletePartitions => {
            let contract = DeletePartitionsTransactionActionGrpcModel::deserialize(content)?;
            let result = TransactionalOperationStep::DeletePartitions {
                table_name: contract.table_name,
                partition_keys: contract.partition_keys,
            };
            Ok(result)
        }

        TransactionType::DeleteRows => {
            let contract = DeleteRowsTransactionActionGrpcModel::deserialize(content)?;
            let result = TransactionalOperationStep::DeleteRows {
                table_name: contract.table_name,
                partition_key: contract.partition_key,
                row_keys: contract.row_keys,
            };
            Ok(result)
        }

        TransactionType::InsertOrReplaceEntities => {
            let contract = InsertOrReplaceEntitiesTransactionActionGrpcModel::deserialize(content)?;

            let rows_by_partition = contract.to_db_rows(now)?;

            let update_rows_state = UpdateRowsStepState {
                table_name: contract.table_name,
                rows_by_partition,
            };

            let result = TransactionalOperationStep::UpdateRows(update_rows_state);

            Ok(result)
        }
    }
}
