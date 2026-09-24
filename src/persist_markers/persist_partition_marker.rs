use std::sync::Arc;

use my_no_sql_sdk::core::db::{DbRow, PartitionKey};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::rust_extensions::sorted_vec::{
    EntityWithStrKey, InsertOrUpdateEntry, SortedVecWithStrKey,
};

use super::PersistRowMarker;

pub struct PersistPartitionMarker {
    pub partition_key: PartitionKey,
    pub persist_whole_partition: Option<DateTimeAsMicroseconds>,
    pub rows_to_persist: SortedVecWithStrKey<PersistRowMarker>,
}

impl EntityWithStrKey for PersistPartitionMarker {
    fn get_key(&self) -> &str {
        self.partition_key.as_str()
    }
}

impl PersistPartitionMarker {
    pub fn new(partition_key: PartitionKey) -> Self {
        Self {
            partition_key,
            persist_whole_partition: None,
            rows_to_persist: SortedVecWithStrKey::new(),
        }
    }

    pub fn persist_row(&mut self, db_row: &Arc<DbRow>, persist_moment: DateTimeAsMicroseconds) {
        match self.rows_to_persist.insert_or_update(db_row.get_row_key()) {
            InsertOrUpdateEntry::Insert(insert_entity) => {
                insert_entity.insert(PersistRowMarker::new(db_row.clone(), persist_moment));
            }
            InsertOrUpdateEntry::Update(update_entry) => {
                if update_entry.item.persist_moment > persist_moment {
                    update_entry.item.persist_moment = persist_moment;
                }

                update_entry.item.db_row = db_row.clone();
            }
        }
    }

    pub fn get_row_to_sync(&self, now: Option<DateTimeAsMicroseconds>) -> Vec<Arc<DbRow>> {
        let mut result = Vec::new();
        for item in self.rows_to_persist.iter() {
            if let Some(now) = now {
                if item.persist_moment.unix_microseconds <= now.unix_microseconds {
                    result.push(item.db_row.clone());
                }
            } else {
                result.push(item.db_row.clone());
            }
        }

        result
    }

    pub fn clean_when_synching_whole_partition(&mut self) {
        self.persist_whole_partition = None;
        self.rows_to_persist.clear(None);
    }

    pub fn clean_when_syncing_rows(&mut self, rows: &[Arc<DbRow>]) {
        for row in rows {
            self.rows_to_persist.remove(row.get_row_key());
        }
    }

    /*
    pub fn mark_table_to_persist(&mut self, moment: DateTimeAsMicroseconds) {
        if self.persist_whole_table.is_none() {
            self.persist_whole_table = Some(moment);
            return;
        }

        let persist_whole_table = self.persist_whole_table.unwrap();

        if persist_whole_table.unix_microseconds > moment.unix_microseconds {
            self.persist_whole_table = Some(moment)
        }
    }

    pub fn mark_partition_to_persist(
        &mut self,
        partition_key: &impl PartitionKeyParameter,
        persist_moment: DateTimeAsMicroseconds,
    ) {
        match self.partitions.insert_or_update(partition_key.as_str()) {
            my_no_sql_sdk::core::rust_extensions::sorted_vec::InsertOrUpdateEntry::Insert(
                entry,
            ) => {
                let entity = PartitionPersistMoment {
                    partition_key: partition_key.to_partition_key(),
                    persist_moment,
                };

                entry.insert(entity);
            }
            my_no_sql_sdk::core::rust_extensions::sorted_vec::InsertOrUpdateEntry::Update(
                entry,
            ) => {
                if persist_moment < entry.item.persist_moment {
                    entry.item.persist_moment = persist_moment;
                }
            }
        }
    }

    pub fn mark_persist_attrs(&mut self) {
        self.persist_attrs = true;
    }

    fn get_partition_ready_to_persist(
        &mut self,
        now: DateTimeAsMicroseconds,
        is_shutting_down: bool,
    ) -> Option<PartitionKey> {
        for item in self.partitions.iter() {
            if is_shutting_down || item.persist_moment.unix_microseconds <= now.unix_microseconds {
                return Some(item.partition_key.clone());
            }
        }
        None
    }

    pub fn get_next_persist_time(&self) -> Option<DateTimeAsMicroseconds> {
        if let Some(persist_whole_table) = self.persist_whole_table {
            return Some(persist_whole_table);
        }

        let mut result: Option<DateTimeAsMicroseconds> = None;

        for partition_dt in self.partitions.iter() {
            match result.clone() {
                Some(current_result) => {
                    if current_result.unix_microseconds
                        > partition_dt.persist_moment.unix_microseconds
                    {
                        result = Some(partition_dt.persist_moment)
                    }
                }
                None => {
                    result = Some(partition_dt.persist_moment);
                }
            }
        }

        result
    }

    pub fn get_what_to_persist(
        &mut self,
        now: DateTimeAsMicroseconds,
        is_shutting_down: bool,
    ) -> Option<PersistResult> {
        if let Some(persist_whole_table) = self.persist_whole_table {
            if persist_whole_table.unix_microseconds <= now.unix_microseconds || is_shutting_down {
                self.persist_whole_table = None;
                self.partitions.clear(Some(16));
                return Some(PersistResult::PersistTable);
            }
        }

        if let Some(partition_key) = self.get_partition_ready_to_persist(now, is_shutting_down) {
            self.partitions.remove(partition_key.as_str());
            return Some(PersistResult::PersistPartition(partition_key));
        }

        if self.persist_attrs {
            self.persist_attrs = false;
            return Some(PersistResult::PersistAttrs);
        }
        None
    }
    */
}

/*
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_add_partition_with_later_date() {
        let mut data_to_persist = PartitionPersistMarker::new();

        data_to_persist
            .mark_partition_to_persist(&"test".to_string(), DateTimeAsMicroseconds::new(5));

        data_to_persist
            .mark_partition_to_persist(&"test".to_string(), DateTimeAsMicroseconds::new(6));

        let result = data_to_persist
            .get_what_to_persist(DateTimeAsMicroseconds::new(5), false)
            .unwrap();

        if let PersistResult::PersistPartition(partition_key) = result {
            assert_eq!("test", partition_key.as_str());
        } else {
            panic!("Should not be here");
        }
    }

    #[test]
    fn test_add_partition_with_table_later() {
        let mut data_to_persist = PartitionPersistMarker::new();

        data_to_persist
            .mark_partition_to_persist(&"test".to_string(), DateTimeAsMicroseconds::new(5));

        data_to_persist.mark_table_to_persist(DateTimeAsMicroseconds::new(6));

        let result = data_to_persist
            .get_what_to_persist(DateTimeAsMicroseconds::new(5), false)
            .unwrap();

        if let PersistResult::PersistPartition(partition_key) = result {
            assert_eq!("test", partition_key.as_str());
        } else {
            panic!("Should not be here");
        }

        let result = data_to_persist.get_what_to_persist(DateTimeAsMicroseconds::new(5), false);

        assert_eq!(true, result.is_none());

        let result = data_to_persist
            .get_what_to_persist(DateTimeAsMicroseconds::new(6), false)
            .unwrap();

        assert_eq!(true, result.is_table());
    }
}
 */
