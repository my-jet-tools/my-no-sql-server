use std::sync::Arc;

use my_no_sql_sdk::core::db::DbRow;

pub enum WriteOperationResult {
    SingleRow(Arc<DbRow>),
    Empty,
}
