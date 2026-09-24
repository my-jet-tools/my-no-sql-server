use std::{collections::BTreeMap, sync::Arc, time::Duration};

use my_no_sql_sdk::core::db::DbNamespaceName;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;

use super::{DataReader, DataReaderConnection};

pub struct DataReadersData {
    tcp: BTreeMap<i32, Arc<DataReader>>,
    http: BTreeMap<String, Arc<DataReader>>,
    all: BTreeMap<String, Arc<DataReader>>,
    id: usize,
}

impl DataReadersData {
    pub fn new() -> Self {
        Self {
            tcp: BTreeMap::new(),
            all: BTreeMap::new(),
            http: BTreeMap::new(),
            id: 0,
        }
    }

    pub fn get_next_id(&mut self) -> usize {
        let result = self.id;
        self.id += 1;
        result
    }

    pub fn insert(&mut self, data_reader: Arc<DataReader>) {
        self.all
            .insert(data_reader.id.to_string(), data_reader.clone());

        match &data_reader.connection {
            DataReaderConnection::Tcp(connection) => {
                self.tcp.insert(connection.get_id(), data_reader);
            }

            DataReaderConnection::Http(connection) => {
                self.http.insert(connection.id.to_string(), data_reader);
            }
        }
    }

    pub fn get_tcp(&self, connection_id: i32) -> Option<Arc<DataReader>> {
        let result = self.tcp.get(&connection_id)?;
        result.clone().into()
    }

    pub fn get_http(&self, session_id: &str) -> Option<Arc<DataReader>> {
        let result = self.http.get(session_id)?;
        result.clone().into()
    }

    pub fn remove_tcp(&mut self, connection_id: i32) -> Option<Arc<DataReader>> {
        if let Some(removed_connection) = self.tcp.remove(&connection_id) {
            return self.all.remove(&removed_connection.id);
        }

        None
    }

    pub fn remove_http(&mut self, data_reader: &DataReader) {
        if let DataReaderConnection::Http(connection) = &data_reader.connection {
            if let Some(removed_connection) = self.http.remove(connection.id.as_str()) {
                self.all.remove(&removed_connection.id);
            }
        }
    }

    pub fn get_all(&self) -> Vec<Arc<DataReader>> {
        self.all.values().map(|itm| itm.clone()).collect()
    }

    fn get_http_sessions_to_gc(
        &self,
        now: DateTimeAsMicroseconds,
        http_timeout: Duration,
    ) -> Option<Vec<Arc<DataReader>>> {
        let mut result = None;

        for http_session in self.http.values() {
            if now
                .duration_since(http_session.get_last_incoming_moment())
                .as_positive_or_zero()
                >= http_timeout
            {
                if result.is_none() {
                    result = Some(Vec::new());
                }

                result.as_mut().unwrap().push(http_session.clone())
            }
        }

        result
    }

    pub fn gc_http_sessions(
        &mut self,
        now: DateTimeAsMicroseconds,
        http_timeout: Duration,
    ) -> Option<Vec<Arc<DataReader>>> {
        if let Some(to_collect) = self.get_http_sessions_to_gc(now, http_timeout) {
            for data_reader in &to_collect {
                self.remove_http(data_reader.as_ref());
            }
            return Some(to_collect);
        }

        None
    }

    pub async fn get_subscribed_to_table(
        &self,
        namespace: &DbNamespaceName,
        table_name: &str,
    ) -> Option<Vec<Arc<DataReader>>> {
        let mut result = None;

        for data_reader in self.all.values() {
            if data_reader.get_namespace() != *namespace {
                continue;
            }

            if data_reader.has_table(table_name).await {
                if result.is_none() {
                    result = Some(Vec::new());
                }

                result.as_mut().unwrap().push(data_reader.clone());
            }
        }

        result
    }
}
