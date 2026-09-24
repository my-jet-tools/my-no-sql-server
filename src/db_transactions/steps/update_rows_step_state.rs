use std::sync::Arc;

use my_no_sql_sdk::core::db::DbRow;

pub struct UpdateRowsStepState {
    pub table_name: String,
    pub rows_by_partition: Vec<(String, Vec<Arc<DbRow>>)>,
}
