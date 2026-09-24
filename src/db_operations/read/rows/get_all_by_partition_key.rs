use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::{DbOperationError, UpdateStatistics},
};

use super::super::ReadOperationResult;

pub async fn get_all_by_partition_key(
    app: &Arc<AppContext>,
    db_table: &Arc<DbTable>,
    partition_key: &String,
    limit: Option<usize>,
    skip: Option<usize>,
    update_statistics: UpdateStatistics,
    now: DateTimeAsMicroseconds,
) -> Result<ReadOperationResult, DbOperationError> {
    super::super::super::check_app_states(app)?;

    let table_data = db_table.data.read();

    let db_partition = table_data.get_partition(partition_key);

    if db_partition.is_none() {
        return Ok(ReadOperationResult::EmptyArray);
    }

    let db_partition = db_partition.unwrap();

    let json_array_writer = super::super::read_filter::filter_and_compile_json(
        db_partition.get_all_rows().into_iter(),
        limit,
        skip,
        |db_row| {
            update_statistics.update(db_table, db_partition, Some(db_row), now);
        },
    );

    return Ok(ReadOperationResult::RowsArray(
        json_array_writer.build().into_bytes(),
    ));
}
