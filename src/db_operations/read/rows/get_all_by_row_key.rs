use std::sync::Arc;

use my_no_sql_sdk::core::my_json::json_writer::JsonArrayWriter;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::{DbOperationError, UpdateStatistics},
};

use super::super::ReadOperationResult;

pub async fn get_all_by_row_key(
    app: &Arc<AppContext>,
    db_table: &Arc<DbTable>,
    row_key: &str,
    limit: Option<usize>,
    skip: Option<usize>,
    update_statistics: UpdateStatistics,
    now: DateTimeAsMicroseconds,
) -> Result<ReadOperationResult, DbOperationError> {
    super::super::super::check_app_states(app)?;

    let table_data = db_table.data.read();

    let mut json_array_writer = JsonArrayWriter::new();
    for (db_partition, db_row) in table_data.get_by_row_key(row_key, skip, limit) {
        update_statistics.update(db_table, db_partition, Some(db_row), now);
        json_array_writer = json_array_writer.write(db_row.as_ref());
    }

    return Ok(ReadOperationResult::RowsArray(
        json_array_writer.build().into_bytes(),
    ));
}
