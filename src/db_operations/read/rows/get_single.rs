use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::{DbOperationError, UpdateStatistics},
};

use super::super::ReadOperationResult;

pub async fn get_single(
    app: &Arc<AppContext>,
    db_table_wrapper: &Arc<DbTable>,
    partition_key: &String,
    row_key: &String,
    update_statistics: UpdateStatistics,
    now: DateTimeAsMicroseconds,
) -> Result<ReadOperationResult, DbOperationError> {
    super::super::super::check_app_states(app)?;

    let table_data = db_table_wrapper.data.read();

    let db_partition = table_data.get_partition(partition_key);

    if db_partition.is_none() {
        return Err(DbOperationError::RecordNotFound);
    }

    let db_partition = db_partition.unwrap();

    let db_row = db_partition.get_row(row_key);

    if db_row.is_none() {
        return Err(DbOperationError::RecordNotFound);
    }

    let db_row = db_row.unwrap();

    update_statistics.update(db_table_wrapper, db_partition, Some(db_row), now);

    return Ok(ReadOperationResult::SingleRow(db_row.to_vec()));
}
