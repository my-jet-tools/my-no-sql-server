use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

pub async fn update_row_keys_last_read_access_time<'s, TRowKeys: Iterator<Item = &'s str>>(
    db_table: &Arc<DbTable>,
    partition_key: &str,
    row_keys: TRowKeys,
) {
    let now = DateTimeAsMicroseconds::now();
    let db_table_access = db_table.data.read();

    if let Some(db_partition) = db_table_access.get_partition(partition_key) {
        for row_key in row_keys {
            if let Some(db_row) = db_partition.get_row(row_key) {
                db_row.update_last_read_access(now);
            }
        }
    }
}
