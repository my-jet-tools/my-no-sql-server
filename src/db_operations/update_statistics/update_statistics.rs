use std::sync::Arc;

use my_no_sql_sdk::core::db::{DbPartition, DbRow};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

#[derive(Clone, Debug)]
pub struct UpdateStatistics {
    pub update_partition_last_read_access_time: bool,
    pub update_rows_last_read_access_time: bool,
    pub update_partition_expiration_time: Option<Option<DateTimeAsMicroseconds>>,
    pub update_rows_expiration_time: Option<Option<DateTimeAsMicroseconds>>,
}

impl UpdateStatistics {
    pub fn update(
        &self,
        db_table: &Arc<DbTable>,
        db_partition: &DbPartition,
        db_row: Option<&Arc<DbRow>>,
        now: DateTimeAsMicroseconds,
    ) {
        if self.update_partition_last_read_access_time {
            db_partition.update_last_read_moment(now);
        }

        if let Some(update_partition_expiration_time) = self.update_partition_expiration_time {
            crate::db_operations::update_statistics::update_partition_expiration_time(
                db_table,
                db_partition.partition_key.to_string(),
                update_partition_expiration_time,
            );
        }

        if let Some(db_row) = db_row {
            if self.update_rows_last_read_access_time {
                db_row.update_last_read_access(now);
            }

            if let Some(update_rows_expiration_time) = self.update_rows_expiration_time {
                db_row.update_expires(update_rows_expiration_time);
            }
        }
    }
    /*
    pub fn has_statistics_to_update(&self) -> bool {
        self.update_partition_last_read_access_time
            || self.update_rows_last_read_access_time
            || self.update_partition_expiration_time.is_some()
            || self.update_rows_expiration_time.is_some()
    }

    pub async fn update_statistics<'s, TRowKeys: Iterator<Item = &'s str>>(
        &self,
        app: &Arc<AppContext>,
        db_table: &Arc<DbTableWrapper>,
        partition_key: &str,
        get_db_rows: impl Fn() -> TRowKeys,
    ) {
        if self.update_partition_last_read_access_time {
            crate::db_operations::update_statistics::update_partition_last_read_time(
                db_table,
                partition_key,
            )
            .await;
        }

        if self.update_rows_last_read_access_time {
            crate::db_operations::update_statistics::update_row_keys_last_read_access_time(
                db_table,
                &partition_key,
                get_db_rows(),
            )
            .await;
        }

        if let Some(set_partition_expiration_time) = self.update_partition_expiration_time {
            crate::db_operations::update_statistics::update_partition_expiration_time(
                app,
                db_table,
                partition_key,
                set_partition_expiration_time,
            );
        }

        if let Some(update_rows_expiration_time) = self.update_rows_expiration_time {
            crate::db_operations::update_statistics::update_rows_expiration_time(
                app,
                db_table,
                partition_key,
                get_db_rows(),
                update_rows_expiration_time,
            );
        }

    }
    */
}
