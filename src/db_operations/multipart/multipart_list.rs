use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    time::Duration,
};

use my_no_sql_sdk::core::db::DbRow;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::db_snapshots::DbRowsSnapshot;
use tokio::sync::Mutex;

use super::Multipart;

pub struct MultipartList {
    items: Mutex<BTreeMap<i64, Multipart>>,
}

impl MultipartList {
    pub fn new() -> Self {
        Self {
            items: Mutex::new(BTreeMap::new()),
        }
    }

    pub async fn add(&self, items: VecDeque<Arc<DbRow>>) -> i64 {
        let mut list_items = self.items.lock().await;
        let mut now = DateTimeAsMicroseconds::now();

        while list_items.contains_key(&now.unix_microseconds) {
            now.unix_microseconds += 1;
        }

        let item = Multipart::new(now, items);

        let id = item.id;

        list_items.insert(item.id, item);

        return id;
    }

    pub async fn get(&self, id: i64, amount: usize) -> Option<DbRowsSnapshot> {
        let mut write_access = self.items.lock().await;

        let (result, delete_it) = {
            let multipart = write_access.get_mut(&id)?;

            let mut result = DbRowsSnapshot::with_capacity(amount);

            while let Some(db_row) = multipart.items.remove(0) {
                result.push(db_row);

                if result.len() >= amount {
                    break;
                }
            }

            (result, multipart.items.len() == 0)
        };

        if delete_it {
            write_access.remove(&id);
        }

        if result.len() == 0 {
            return None;
        }

        return Some(result);
    }

    pub async fn gc(&self, now: DateTimeAsMicroseconds, timeout: Duration) {
        let mut write_access = self.items.lock().await;

        let items_to_gs = {
            let mut result = None;

            for multipart in write_access.values() {
                if now.duration_since(multipart.created).as_positive_or_zero() >= timeout {
                    if result.is_none() {
                        result = Some(Vec::new());
                    }

                    result.as_mut().unwrap().push(multipart.id)
                }
            }

            result
        };

        if let Some(items_to_gs) = items_to_gs {
            for item_to_gs in items_to_gs {
                write_access.remove(&item_to_gs);
            }
        }
    }
}
