use std::sync::Arc;

use my_no_sql_sdk::core::db::{DbPartition, DbRow};
use my_no_sql_server_core::DbTableWrapper;
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{
    app::AppContext,
    db_operations::{DbOperationError, UpdateStatistics},
};

pub async fn update_expiration_time(
    app: &Arc<AppContext>,
    db_table: &Arc<DbTableWrapper>,
    partition_key: &str,
    db_rows: &[&Arc<DbRow>],
    now: DateTimeAsMicroseconds,
    update_statistics: &UpdateStatistics,
) -> Result<(), DbOperationError> {
    if !update_statistics.has_statistics_to_update() {
        return Ok(());
    }

    super::check_app_states(app)?;
    let table_data = db_table.data.read();

    let db_partition = table_data.get_partition(&partition_key);

    if db_partition.is_none() {
        return Ok(());
    }

    execute(db_partition.unwrap(), db_rows, update_statistics, now).await;
    Ok(())
}

pub async fn execute(
    db_partition: &DbPartition,
    db_rows: &[&Arc<DbRow>],
    update_statistics: &UpdateStatistics,
    now: DateTimeAsMicroseconds,
) {
    let now = DateTimeAsMicroseconds::now();

    if update_statistics.update_partition_last_read_access_time {
        db_partition.last_read_moment.update(now);
    }

    if update_statistics.update_rows_last_read_access_time {
        for db_row in db_rows {
            db_row.update_last_read_access(now)
        }
    }
}
