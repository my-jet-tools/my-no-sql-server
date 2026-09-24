use my_no_sql_sdk::core::db::{DbTableInner, DbTableName};

pub struct SyncTableData {
    pub table_name: DbTableName,
}

impl SyncTableData {
    pub fn new(table_data: &DbTableInner) -> Self {
        Self {
            table_name: table_data.name.clone(),
        }
    }
}
