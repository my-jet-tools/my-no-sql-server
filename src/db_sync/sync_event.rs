use super::states::{
    DeleteRowsEventSyncData, DeleteTableSyncData, InitPartitionsSyncEventData,
    InitTableEventSyncData, TableFirstInitSyncData, UpdateRowsSyncData,
    UpdateTableAttributesSyncData,
};

pub enum SyncEvent {
    UpdateTableAttributes(UpdateTableAttributesSyncData),

    InitTable(InitTableEventSyncData),

    InitPartitions(InitPartitionsSyncEventData),

    UpdateRows(UpdateRowsSyncData),

    DeleteRows(DeleteRowsEventSyncData),

    DeleteTable(DeleteTableSyncData),

    TableFirstInit(TableFirstInitSyncData),
}

impl SyncEvent {
    pub fn get_table_name(&self) -> &str {
        match self {
            SyncEvent::UpdateTableAttributes(data) => data.table_data.table_name.as_str(),
            SyncEvent::InitTable(data) => data.table_data.table_name.as_str(),
            SyncEvent::InitPartitions(data) => data.table_data.table_name.as_str(),
            SyncEvent::UpdateRows(data) => data.table_data.table_name.as_str(),
            SyncEvent::DeleteRows(data) => data.table_data.table_name.as_str(),
            SyncEvent::DeleteTable(data) => data.table_data.table_name.as_str(),
            SyncEvent::TableFirstInit(data) => data.db_table.name.as_str(),
        }
    }
}

impl Into<SyncEvent> for InitTableEventSyncData {
    fn into(self) -> SyncEvent {
        SyncEvent::InitTable(self)
    }
}
