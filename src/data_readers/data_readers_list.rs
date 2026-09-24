use std::{sync::Arc, time::Duration};

use my_no_sql_sdk::core::db::DbNamespaceName;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::sync::RwLock;

use crate::tcp::MyNoSqlTcpConnection;

use super::{
    http_connection::HttpConnectionInfo,
    tcp_connection::{ReaderName, TcpConnectionInfo},
    DataReader, DataReaderConnection, DataReadersData,
};

pub struct DataReadersList {
    data: RwLock<DataReadersData>,
    http_session_time_out: Duration,
}

impl DataReadersList {
    pub fn new(http_session_time_out: Duration) -> Self {
        Self {
            data: RwLock::new(DataReadersData::new()),
            http_session_time_out,
        }
    }

    pub async fn add_tcp(
        &self,
        tcp_connection: Arc<MyNoSqlTcpConnection>,
        name: ReaderName,
        compress_data: bool,
    ) {
        let id = format!("Tcp-{}", tcp_connection.id);

        let connection_info = TcpConnectionInfo::new(tcp_connection, name, compress_data);

        let connection_info = Arc::new(connection_info);

        let mut write_lock = self.data.write().await;

        let data_reader = DataReader::new(id, DataReaderConnection::Tcp(connection_info));
        write_lock.insert(Arc::new(data_reader));
    }

    pub async fn add_http(&self, name: String, version: String, ip: String) -> Arc<DataReader> {
        let mut write_lock = self.data.write().await;
        let id = format!("Http-{}", write_lock.get_next_id());

        let http_connection_info = HttpConnectionInfo::new(id.to_string(), name, version, ip);

        let data_reader = Arc::new(DataReader::new(
            id.clone(),
            DataReaderConnection::Http(http_connection_info),
        ));

        write_lock.insert(data_reader.clone());

        data_reader
    }

    pub async fn get_tcp(&self, tcp_connection: &MyNoSqlTcpConnection) -> Option<Arc<DataReader>> {
        let read_lock = self.data.read().await;
        read_lock.get_tcp(tcp_connection.id)
    }

    pub async fn get_http(&self, session_id: &str) -> Option<Arc<DataReader>> {
        let read_lock = self.data.read().await;
        read_lock.get_http(session_id)
    }

    pub async fn remove_tcp(
        &self,
        tcp_connection: &MyNoSqlTcpConnection,
    ) -> Option<Arc<DataReader>> {
        println!("Tcp reader is disconnected {}", tcp_connection.id);
        let mut write_lock = self.data.write().await;
        write_lock.remove_tcp(tcp_connection.id)
    }

    pub async fn get_all(&self) -> Vec<Arc<DataReader>> {
        let read_lock = self.data.read().await;
        read_lock.get_all()
    }

    pub async fn get_subscribed_to_table(
        &self,
        namespace: &DbNamespaceName,
        table_name: &str,
    ) -> Option<Vec<Arc<DataReader>>> {
        let read_access = self.data.read().await;
        read_access
            .get_subscribed_to_table(namespace, table_name)
            .await
    }

    pub async fn gc_http_sessions(
        &self,
        now: DateTimeAsMicroseconds,
    ) -> Option<Vec<Arc<DataReader>>> {
        let mut write_access = self.data.write().await;
        write_access.gc_http_sessions(now, self.http_session_time_out)
    }
}
