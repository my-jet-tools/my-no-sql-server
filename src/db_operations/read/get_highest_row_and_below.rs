use std::sync::Arc;

use my_no_sql_sdk::core::my_json::json_writer::JsonArrayWriter;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::{DbOperationError, UpdateStatistics},
};

use super::ReadOperationResult;

pub async fn get_highest_row_and_below(
    app: &Arc<AppContext>,
    db_table_wrapper: &Arc<DbTable>,
    partition_key: &String,
    row_key: &String,
    limit: Option<usize>,
    update_statistics: UpdateStatistics,
    now: DateTimeAsMicroseconds,
) -> Result<ReadOperationResult, DbOperationError> {
    super::super::check_app_states(app)?;

    let read_access = db_table_wrapper.data.read();

    let db_partition = read_access.get_partition(partition_key);

    if db_partition.is_none() {
        return Ok(ReadOperationResult::EmptyArray);
    }

    let db_partition = db_partition.unwrap();

    let mut json_array_writer = JsonArrayWriter::new();
    let mut count = 0;
    for db_row in db_partition.get_highest_row_and_below(row_key) {
        if let Some(limit) = limit {
            if count >= limit {
                break;
            }
        }
        update_statistics.update(db_table_wrapper, db_partition, Some(db_row), now);
        json_array_writer = json_array_writer.write(db_row.as_ref());

        count += 1;
    }

    return Ok(ReadOperationResult::RowsArray(
        json_array_writer.build().into_bytes(),
    ));
}

/*
async fn get_highest_row_and_below_with_no_expiration_time_update(
    db_table: &DbTable,
    partition_key: &str,
    row_key: &String,
    limit: Option<usize>,
) -> ReadOperationResult {
    let now = DateTimeAsMicroseconds::now();

    let read_access = db_table.data.read().await;

    let db_partition = read_access.get_partition(partition_key);

    if db_partition.is_none() {
        return ReadOperationResult::EmptyArray;
    }

    let db_partition = db_partition.unwrap();

    let db_rows = db_partition.get_highest_row_and_below(row_key, Some(now), limit);

    if db_rows.len() == 0 {
        return ReadOperationResult::EmptyArray;
    }

    let mut json_array_writer = JsonArrayWriter::new();

    for db_row in db_rows {
        json_array_writer.write_raw_element(db_row.data.as_ref());
    }

    return ReadOperationResult::RowsArray(json_array_writer.build());
}

async fn get_highest_row_and_below_and_update_expiration_time(
    db_table: &DbTableWrapper,
    partition_key: &str,
    row_key: &String,
    limit: Option<usize>,
    update_expiration_time: UpdateExpirationTimeModel,
) -> ReadOperationResult {
}
 */
