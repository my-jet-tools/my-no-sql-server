use std::{sync::Arc, time::Duration};

use my_no_sql_sdk::core::db::{DbRow, DbTableName, PartitionKey};
use my_no_sql_sdk::server::rust_extensions::{date_time::DateTimeAsMicroseconds, sorted_vec::*};

use super::{PersistByTableItem, PersistTask};

pub struct PersistMarkersInner {
    items: SortedVecWithStrKey<PersistByTableItem>,
}

impl PersistMarkersInner {
    pub fn new() -> Self {
        Self {
            items: SortedVecWithStrKey::new(),
        }
    }

    fn get_item_mut(&mut self, table_name: &DbTableName) -> &mut PersistByTableItem {
        let index = match self.items.insert_or_if_not_exists(table_name.as_str()) {
            InsertIfNotExists::Insert(insert_entity) => {
                let index = insert_entity.index;
                let item = PersistByTableItem::new(table_name.clone());
                insert_entity.insert(item);
                index
            }
            InsertIfNotExists::Exists(index) => index,
        };

        self.items.get_by_index_mut(index).unwrap()
    }

    pub fn persist_table_content(
        &mut self,
        table_name: &DbTableName,
        persist_moment: DateTimeAsMicroseconds,
    ) {
        let item = self.get_item_mut(table_name);

        match item.persist_whole_table_content {
            Some(date_time) => {
                if persist_moment < date_time {
                    item.persist_whole_table_content = Some(persist_moment);
                }
            }
            None => {
                item.persist_whole_table_content = Some(persist_moment);
            }
        }
    }

    pub fn persist_table_attributes(
        &mut self,
        table_name: &DbTableName,
        persist_moment: DateTimeAsMicroseconds,
    ) {
        let item = self.get_item_mut(table_name);

        match item.persist_table_attributes {
            Some(date_time) => {
                if persist_moment < date_time {
                    item.persist_table_attributes = Some(persist_moment);
                }
            }
            None => {
                item.persist_table_attributes = Some(persist_moment);
            }
        }
    }

    pub fn persist_rows<'s>(
        &mut self,
        table_name: &DbTableName,
        partition_key: &PartitionKey,
        moment: DateTimeAsMicroseconds,
        rows_to_persist: impl Iterator<Item = &'s Arc<DbRow>>,
    ) {
        let item = self.get_item_mut(table_name);

        item.persist_rows(partition_key, moment, rows_to_persist);
    }

    pub fn persist_whole_partition(
        &mut self,
        table_name: &DbTableName,
        partition_key: &PartitionKey,
        persist_moment: DateTimeAsMicroseconds,
    ) {
        let item = self.get_item_mut(table_name);

        item.persist_whole_partition(partition_key, persist_moment);
    }

    pub fn get_by_table_mut(
        &mut self,
        table_name: &DbTableName,
    ) -> Option<&mut PersistByTableItem> {
        self.items.get_mut(table_name.as_str())
    }

    pub fn get_by_table(&self, table_name: &str) -> Option<&PersistByTableItem> {
        self.items.get(table_name)
    }

    pub fn get_persist_task(&self, now: Option<DateTimeAsMicroseconds>) -> Option<PersistTask> {
        for itm in self.items.iter() {
            let persist_task = itm.get_persist_task(now);

            if persist_task.is_some() {
                return persist_task;
            }
        }

        None
    }

    pub fn set_last_persist_time(
        &mut self,
        table_name: &DbTableName,
        moment: DateTimeAsMicroseconds,
        duration: Duration,
    ) {
        let item = self.get_item_mut(table_name);
        item.metrics.update(moment, duration);
    }

    pub fn get_amount_to_persist(&self) -> usize {
        self.items
            .iter()
            .map(|item| item.get_amount_to_persist())
            .sum()
    }

    pub fn has_something_to_persist(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.has_something_to_persist())
    }
}
