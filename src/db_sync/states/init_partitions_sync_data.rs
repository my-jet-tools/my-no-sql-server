use my_no_sql_sdk::core::db::{DbTableInner, PartitionKey};
use my_no_sql_sdk::core::my_json::json_writer::{EmptyJsonArray, JsonObjectWriter};
use my_no_sql_sdk::core::rust_extensions::sorted_vec::{EntityWithStrKey, SortedVecWithStrKey};
use my_no_sql_sdk::server::db_snapshots::DbPartitionSnapshot;

use crate::db_sync::EventSource;

use super::SyncTableData;

pub struct InitPartitionsSyncData {
    pub partition_key: PartitionKey,
    pub snapshot: Option<DbPartitionSnapshot>,
}

impl EntityWithStrKey for InitPartitionsSyncData {
    fn get_key(&self) -> &str {
        self.partition_key.as_str()
    }
}

pub struct InitPartitionsSyncEventData {
    pub table_data: SyncTableData,
    #[allow(dead_code)]
    pub event_src: EventSource,
    pub partitions_to_update: SortedVecWithStrKey<InitPartitionsSyncData>,
}

impl InitPartitionsSyncEventData {
    pub fn new(table_data: &DbTableInner, event_src: EventSource) -> Self {
        Self {
            table_data: SyncTableData::new(table_data),
            event_src,
            partitions_to_update: SortedVecWithStrKey::new(),
        }
    }

    pub fn new_as_update_partition(
        db_table: &DbTableInner,
        partition_key: PartitionKey,
        event_src: EventSource,
    ) -> Self {
        let mut partitions_to_update = SortedVecWithStrKey::new();

        if let Some(db_partition) = db_table.get_partition(partition_key.as_str()) {
            partitions_to_update.insert_or_replace(InitPartitionsSyncData {
                partition_key,
                snapshot: Some(db_partition.into()),
            });
        } else {
            partitions_to_update.insert_or_replace(InitPartitionsSyncData {
                partition_key,
                snapshot: None,
            });
        }

        Self {
            table_data: SyncTableData::new(db_table),
            event_src,
            partitions_to_update,
        }
    }

    pub fn add(&mut self, partition_key: PartitionKey, snapshot: Option<DbPartitionSnapshot>) {
        self.partitions_to_update
            .insert_or_replace(InitPartitionsSyncData {
                partition_key,
                snapshot,
            });
    }

    pub fn as_json(&self) -> JsonObjectWriter {
        let mut json_object_writer = JsonObjectWriter::new();

        for db_partition in self.partitions_to_update.iter() {
            if let Some(db_partition_snapshot) = &db_partition.snapshot {
                json_object_writer = json_object_writer.write(
                    db_partition.partition_key.as_str(),
                    db_partition_snapshot.db_rows_snapshot.as_json_array(),
                );
            } else {
                json_object_writer =
                    json_object_writer.write(db_partition.partition_key.as_str(), EmptyJsonArray)
            }
        }

        json_object_writer
    }
}
