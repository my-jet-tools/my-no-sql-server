use crate::db_sync::EventSource;

use super::SyncTableData;

pub struct UpdateTableAttributesSyncData {
    pub table_data: SyncTableData,
    #[allow(dead_code)]
    pub event_src: EventSource,
}
