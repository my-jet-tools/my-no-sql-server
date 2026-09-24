use std::sync::Arc;

use my_no_sql_sdk::core::db::{DbRow, DbTableName, PartitionKey};
use my_no_sql_sdk::server::rust_extensions::{date_time::DateTimeAsMicroseconds, sorted_vec::*};

use super::{PersistMetrics, PersistPartitionMarker, PersistTask};

pub struct PersistByTableItem {
    pub table_name: DbTableName,
    pub persist_whole_table_content: Option<DateTimeAsMicroseconds>,
    pub persist_table_attributes: Option<DateTimeAsMicroseconds>,
    pub persist_partitions: SortedVecWithStrKey<PersistPartitionMarker>,
    pub metrics: PersistMetrics,
}

impl PersistByTableItem {
    pub fn new(table_name: DbTableName) -> Self {
        Self {
            table_name,
            persist_whole_table_content: None,
            persist_table_attributes: None,
            persist_partitions: SortedVecWithStrKey::new(),
            metrics: PersistMetrics::default(),
        }
    }

    fn get_or_create_partition_item(
        &mut self,
        partition_key: &PartitionKey,
    ) -> &mut PersistPartitionMarker {
        let index = match self
            .persist_partitions
            .insert_or_if_not_exists(partition_key.as_str())
        {
            InsertIfNotExists::Insert(insert_entity) => {
                let index = insert_entity.index;
                let item = PersistPartitionMarker::new(partition_key.clone());
                insert_entity.insert(item);
                index
            }
            InsertIfNotExists::Exists(index) => index,
        };

        self.persist_partitions.get_by_index_mut(index).unwrap()
    }

    pub fn persist_rows<'s>(
        &mut self,
        partition_key: &PartitionKey,
        moment: DateTimeAsMicroseconds,
        db_rows: impl Iterator<Item = &'s Arc<DbRow>>,
    ) {
        let by_partition = self.get_or_create_partition_item(partition_key);

        for db_row in db_rows {
            by_partition.persist_row(db_row, moment);
        }
    }

    pub fn persist_whole_partition(
        &mut self,
        partition_key: &PartitionKey,
        persist_moment: DateTimeAsMicroseconds,
    ) {
        let by_partition = self.get_or_create_partition_item(partition_key);

        match by_partition.persist_whole_partition {
            Some(current_moment) => {
                if current_moment > persist_moment {
                    by_partition.persist_whole_partition = Some(persist_moment);
                }
            }
            None => {
                by_partition.persist_whole_partition = Some(persist_moment);
            }
        }
    }

    fn get_partition_to_sync(&self, now: Option<DateTimeAsMicroseconds>) -> Option<PartitionKey> {
        for partition in self.persist_partitions.iter() {
            if let Some(persist_moment) = partition.persist_whole_partition {
                if let Some(now) = now {
                    if persist_moment < now {
                        return Some(partition.partition_key.clone());
                    }
                } else {
                    return Some(partition.partition_key.clone());
                }
            }
        }

        None
    }

    pub fn get_persist_task(&self, now: Option<DateTimeAsMicroseconds>) -> Option<PersistTask> {
        if let Some(persist_moment) = self.persist_table_attributes {
            if let Some(now) = now {
                if persist_moment.unix_microseconds < now.unix_microseconds {
                    return Some(PersistTask::SaveTableAttributes(self.table_name.clone()));
                }
            } else {
                return Some(PersistTask::SaveTableAttributes(self.table_name.clone()));
            }
        }

        if let Some(persist_moment) = self.persist_whole_table_content {
            if let Some(now) = now {
                if persist_moment.unix_microseconds < now.unix_microseconds {
                    return Some(PersistTask::SyncTable(self.table_name.clone()));
                }
            } else {
                return Some(PersistTask::SyncTable(self.table_name.clone()));
            }
        }

        if let Some(partition_key) = self.get_partition_to_sync(now) {
            return Some(PersistTask::SyncPartition {
                table_name: self.table_name.clone(),
                partition_key: partition_key,
            });
        }

        let mut jobs = Vec::new();

        for partition in self.persist_partitions.iter() {
            let items = partition.get_row_to_sync(now);

            if items.len() > 0 {
                jobs.push(super::SyncRowJobDescription {
                    partition_key: partition.partition_key.clone(),
                    items,
                });
            }
        }

        if jobs.len() > 0 {
            return Some(PersistTask::SyncRows {
                table_name: self.table_name.clone(),
                jobs,
            });
        }

        None
    }

    pub fn clean_when_synching_whole_table(&mut self) {
        self.persist_whole_table_content = None;
        self.persist_partitions.clear(None);
    }

    pub fn clean_when_synching_whole_partition(&mut self, partition_key: &PartitionKey) {
        self.persist_partitions
            .get_mut(partition_key.as_str())
            .unwrap()
            .clean_when_synching_whole_partition();
    }

    pub fn clean_when_syncing_rows(&mut self, partition_key: &PartitionKey, rows: &[Arc<DbRow>]) {
        self.persist_partitions
            .get_mut(partition_key.as_str())
            .unwrap()
            .clean_when_syncing_rows(rows);
    }

    pub fn get_metrics(&self) -> PersistMetrics {
        let mut result = self.metrics.clone();

        // Counted on read rather than tracked: the queue is the only thing that
        // knows what is still owed.
        result.persist_amount = self.get_amount_to_persist();

        if let Some(value) = self.persist_whole_table_content {
            result.next_persist_time = Some(value);
            return result;
        }

        if let Some(value) = self.persist_table_attributes {
            result.next_persist_time = Some(value);
            return result;
        }

        for partition in self.persist_partitions.iter() {
            if let Some(value) = partition.persist_whole_partition {
                result.next_persist_time = Some(value);
                return result;
            }

            for row in partition.rows_to_persist.iter() {
                match result.next_persist_time {
                    Some(current) => {
                        if row.persist_moment < current {
                            result.next_persist_time = Some(row.persist_moment);
                        }
                    }
                    None => {
                        result.next_persist_time = Some(row.persist_moment);
                    }
                }
            }
        }

        result
    }

    /// How many writes the table is still waiting for: its content or its
    /// attributes, every partition queued whole, and every row queued on its own.
    pub fn get_amount_to_persist(&self) -> usize {
        let mut result = 0;

        if self.persist_whole_table_content.is_some() {
            result += 1;
        }

        if self.persist_table_attributes.is_some() {
            result += 1;
        }

        for partition in self.persist_partitions.iter() {
            if partition.persist_whole_partition.is_some() {
                result += 1;
            }

            result += partition.rows_to_persist.len();
        }

        result
    }

    pub fn has_something_to_persist(&self) -> bool {
        if self.persist_whole_table_content.is_some() {
            return true;
        }

        if self.persist_table_attributes.is_some() {
            return true;
        }

        for partition in self.persist_partitions.iter() {
            if partition.persist_whole_partition.is_some() {
                return true;
            }

            if partition.rows_to_persist.len() > 0 {
                return true;
            }
        }

        false
    }
}

impl EntityWithStrKey for PersistByTableItem {
    fn get_key(&self) -> &str {
        self.table_name.as_str()
    }
}
