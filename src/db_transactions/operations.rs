use std::collections::BTreeMap;

use my_no_sql_sdk::core::db::DbNamespaceName;

use super::steps::TransactionalOperationStep;

pub struct TransactionalOperations {
    /// The namespace this transaction was started in. Every table it touches is
    /// resolved inside it — a transaction never spans namespaces.
    pub namespace: DbNamespaceName,
    pub operations: BTreeMap<String, Vec<TransactionalOperationStep>>,
}

impl TransactionalOperations {
    pub fn new(namespace: DbNamespaceName) -> Self {
        Self {
            namespace,
            operations: BTreeMap::new(),
        }
    }

    pub fn add_event(&mut self, event: TransactionalOperationStep) {
        let table_name = event.get_table_name();

        if !self.operations.contains_key(table_name) {
            self.operations.insert(table_name.to_string(), Vec::new());
        }

        self.operations.get_mut(table_name).unwrap().push(event);
    }
}
