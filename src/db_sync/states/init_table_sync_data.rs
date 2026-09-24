use my_no_sql_sdk::core::db::DbTableInner;
use my_no_sql_sdk::server::db_snapshots::DbTableSnapshot;

use crate::db_sync::EventSource;

use super::SyncTableData;

pub struct InitTableEventSyncData {
    pub table_data: SyncTableData,
    #[allow(dead_code)]
    pub event_src: EventSource,
    pub table_snapshot: DbTableSnapshot,
}

impl InitTableEventSyncData {
    pub fn new(db_table: &DbTableInner, event_src: EventSource) -> Self {
        Self {
            table_data: SyncTableData::new(db_table),
            event_src,
            table_snapshot: DbTableSnapshot::new(db_table.get_last_write_moment(), db_table),
        }
    }
}
