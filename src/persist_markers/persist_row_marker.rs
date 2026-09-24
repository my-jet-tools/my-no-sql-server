use std::sync::Arc;

use my_no_sql_sdk::core::db::DbRow;
use my_no_sql_sdk::server::rust_extensions::{
    date_time::DateTimeAsMicroseconds, sorted_vec::EntityWithStrKey,
};

pub struct PersistRowMarker {
    pub db_row: Arc<DbRow>,
    pub persist_moment: DateTimeAsMicroseconds,
}

impl PersistRowMarker {
    pub fn new(db_row: Arc<DbRow>, moment: DateTimeAsMicroseconds) -> Self {
        Self {
            db_row,
            persist_moment: moment,
        }
    }
}

impl EntityWithStrKey for PersistRowMarker {
    fn get_key(&self) -> &str {
        self.db_row.get_row_key()
    }
}
