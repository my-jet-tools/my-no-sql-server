use super::UpdateRowsStepState;

pub enum TransactionalOperationStep {
    CleanTable {
        table_name: String,
    },
    DeletePartitions {
        table_name: String,
        partition_keys: Vec<String>,
    },

    DeleteRows {
        table_name: String,
        partition_key: String,
        row_keys: Vec<String>,
    },

    UpdateRows(UpdateRowsStepState),
}

impl TransactionalOperationStep {
    pub fn get_table_name(&self) -> &str {
        match self {
            TransactionalOperationStep::CleanTable { table_name } => table_name,
            TransactionalOperationStep::DeletePartitions {
                table_name,
                partition_keys: _,
            } => table_name,
            TransactionalOperationStep::DeleteRows {
                table_name,
                partition_key: _,
                row_keys: _,
            } => table_name,
            TransactionalOperationStep::UpdateRows(state) => state.table_name.as_str(),
        }
    }
}
