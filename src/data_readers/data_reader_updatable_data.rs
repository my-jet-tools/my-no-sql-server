use std::{collections::BTreeMap, sync::Arc};

use my_no_sql_sdk::server::DbTable;

pub struct DataReaderUpdatableData {
    tables: BTreeMap<String, Arc<DbTable>>,
}

impl DataReaderUpdatableData {
    pub fn new() -> Self {
        Self {
            tables: BTreeMap::new(),
        }
    }

    pub fn subscribe(&mut self, db_table_wrapper: &Arc<DbTable>) {
        self.tables
            .insert(db_table_wrapper.name.to_string(), db_table_wrapper.clone());
    }

    pub fn unsubscribe(&mut self, table_name: &str) {
        self.tables.remove(table_name);
    }

    pub fn has_table(&self, table_name: &str) -> bool {
        self.tables.contains_key(table_name)
    }

    pub fn has_any_subscription(&self) -> bool {
        !self.tables.is_empty()
    }

    pub fn get_table_names(&self) -> Vec<String> {
        self.tables.keys().map(|id| id.to_string()).collect()
    }
}
