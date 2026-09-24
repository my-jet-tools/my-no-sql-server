use std::sync::Arc;

use my_no_sql_sdk::core::my_json::json_writer::JsonArrayWriter;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::{DbOperationError, UpdateStatistics},
};

use super::super::ReadOperationResult;

pub async fn get_all(
    app: &Arc<AppContext>,
    db_table_wrapper: &Arc<DbTable>,
    limit: Option<usize>,
    skip: Option<usize>,
    update_statistics: UpdateStatistics,
    now: DateTimeAsMicroseconds,
) -> Result<ReadOperationResult, DbOperationError> {
    super::super::super::check_app_states(app)?;

    let table_data = db_table_wrapper.data.read();

    let mut json_array_writer = JsonArrayWriter::new();
    for (db_partition, db_row) in table_data.get_all_rows(skip, limit) {
        update_statistics.update(db_table_wrapper, db_partition, Some(db_row), now);
        json_array_writer = json_array_writer.write(db_row.as_ref());
    }

    return Ok(ReadOperationResult::RowsArray(
        json_array_writer.build().into_bytes(),
    ));
}

/*
async fn get_all_and_no_expiration_time_update(
    table: &DbTable,
    limit: Option<usize>,
    skip: Option<usize>,
) -> ReadOperationResult {
    let now = DateTimeAsMicroseconds::now();

    let table_data = table.data.read().await;

    table_data.last_read_time.update(now);

    let mut json_array_writer = JsonArrayWriter::new();

    for db_row in DbRowsFilter::new(table_data.get_all_rows().into_iter(), limit, skip) {
        json_array_writer.write_raw_element(&db_row.data);
        db_row.last_read_access.update(now);
    }

    ReadOperationResult::RowsArray(json_array_writer.build())
}

async fn get_all_and_update_expiration_time(
    table: &DbTable,
    limit: Option<usize>,
    skip: Option<usize>,
    update_statistics: UpdateStatistics,
) -> ReadOperationResult {
    let now = DateTimeAsMicroseconds::now();

    let mut table_data = table.data.write().await;

    table_data.last_read_time.update(now);

    let result_items = table_data.get_all_rows_and_update_expiration_time(update_expiration_time);

    let mut json_array_writer = JsonArrayWriter::new();

    for db_row in DbRowsFilter::new(result_items.iter().map(|itm| itm), limit, skip) {
        json_array_writer.write_raw_element(&db_row.data);
        db_row.last_read_access.update(now);
    }

    ReadOperationResult::RowsArray(json_array_writer.build())
}
*/
