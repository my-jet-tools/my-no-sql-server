use std::sync::Arc;

use my_no_sql_sdk::core::db::{DbRow, DbTableInner, PartitionKey, PartitionKeyParameter};
use my_no_sql_sdk::core::my_json::json_writer::{JsonArrayWriter, JsonNullValue, JsonObjectWriter};
use my_no_sql_sdk::core::rust_extensions::sorted_vec::{EntityWithStrKey, SortedVecWithStrKey};

use crate::db_sync::EventSource;

use super::SyncTableData;

pub struct DeletedRowData {
    pub partition_key: PartitionKey,
    pub db_rows: Vec<Arc<DbRow>>,
}

impl EntityWithStrKey for DeletedRowData {
    fn get_key(&self) -> &str {
        self.partition_key.as_str()
    }
}

pub struct DeleteRowsEventSyncData {
    pub table_data: SyncTableData,
    #[allow(dead_code)]
    pub event_src: EventSource,
    pub deleted_partitions: Option<SortedVecWithStrKey<PartitionKey>>,
    pub deleted_rows: Option<SortedVecWithStrKey<DeletedRowData>>,
}

impl DeleteRowsEventSyncData {
    pub fn new(db_table: &DbTableInner, event_src: EventSource) -> Self {
        Self {
            table_data: SyncTableData::new(db_table),
            event_src,
            deleted_partitions: None,
            deleted_rows: None,
        }
    }

    fn check_that_we_are_in_partition_mode(
        &mut self,
        partition_key: &impl PartitionKeyParameter,
    ) -> &mut SortedVecWithStrKey<DeletedRowData> {
        if let Some(deleted_partitions) = &self.deleted_partitions {
            if deleted_partitions.contains(partition_key.as_str()) {
                panic!(
                    "Can not add deleted rows from partition {}",
                    partition_key.as_str()
                );
            }
        }

        if self.deleted_rows.is_none() {
            self.deleted_rows = Some(SortedVecWithStrKey::new())
        }

        return self.deleted_rows.as_mut().unwrap();
    }

    pub fn add_deleted_row(
        &mut self,
        partition_key: &impl PartitionKeyParameter,
        deleted_row: Arc<DbRow>,
    ) {
        let deleted_rows = self.check_that_we_are_in_partition_mode(partition_key);

        match deleted_rows.insert_or_update(partition_key.as_str()) {
            my_no_sql_sdk::core::rust_extensions::sorted_vec::InsertOrUpdateEntry::Insert(
                entry,
            ) => {
                entry.insert(DeletedRowData {
                    partition_key: partition_key.to_partition_key(),
                    db_rows: vec![deleted_row],
                });
            }
            my_no_sql_sdk::core::rust_extensions::sorted_vec::InsertOrUpdateEntry::Update(
                entry,
            ) => entry.item.db_rows.push(deleted_row),
        }
    }

    pub fn add_deleted_rows(
        &mut self,
        partition_key: &impl PartitionKeyParameter,
        deleted_rows_to_add: &[Arc<DbRow>],
    ) {
        let deleted_rows = self.check_that_we_are_in_partition_mode(partition_key);

        match deleted_rows.insert_or_update(partition_key.as_str()) {
            my_no_sql_sdk::core::rust_extensions::sorted_vec::InsertOrUpdateEntry::Insert(
                entry,
            ) => {
                entry.insert(DeletedRowData {
                    partition_key: partition_key.to_partition_key(),
                    db_rows: deleted_rows_to_add.to_vec(),
                });
            }
            my_no_sql_sdk::core::rust_extensions::sorted_vec::InsertOrUpdateEntry::Update(
                entry,
            ) => entry.item.db_rows.extend_from_slice(deleted_rows_to_add),
        }
    }

    pub fn new_deleted_partition(&mut self, partition_key: &impl PartitionKeyParameter) {
        if let Some(deleted_rows) = &mut self.deleted_rows {
            deleted_rows.remove(partition_key.as_str());
        }

        if self.deleted_partitions.is_none() {
            self.deleted_partitions = Some(SortedVecWithStrKey::new());
        }

        self.deleted_partitions
            .as_mut()
            .unwrap()
            .insert_or_replace(partition_key.to_partition_key());
    }

    pub fn as_vec(&self) -> Vec<u8> {
        let mut json_object_writer = JsonObjectWriter::new();

        {
            if let Some(deleted_partitions) = &self.deleted_partitions {
                for partition_key in deleted_partitions.iter() {
                    json_object_writer =
                        json_object_writer.write(partition_key.as_str(), JsonNullValue);
                }
            }

            if let Some(deleted_rows) = &self.deleted_rows {
                for deleted_rows_data in deleted_rows.iter() {
                    let mut deleted_rows_json_array = JsonArrayWriter::new();
                    for deleted_row in deleted_rows_data.db_rows.iter() {
                        deleted_rows_json_array =
                            deleted_rows_json_array.write(deleted_row.get_row_key());
                    }
                    json_object_writer = json_object_writer.write(
                        deleted_rows_data.partition_key.as_str(),
                        deleted_rows_json_array,
                    );
                }
            }
        }

        json_object_writer.build().into_bytes()
    }
}
