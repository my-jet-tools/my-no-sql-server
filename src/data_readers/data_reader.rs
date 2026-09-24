use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use my_no_sql_sdk::core::db::DbNamespaceName;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;
use tokio::sync::RwLock;

use super::{DataReaderConnection, DataReaderUpdatableData};

pub struct DataReadeMetrics {
    pub session_id: String,
    pub connected: DateTimeAsMicroseconds,
    pub last_incoming_moment: DateTimeAsMicroseconds,
    pub ip: String,
    pub name: String,
    pub tables: Vec<String>,
    pub pending_to_send: usize,
    /// Round trip in microseconds, as the reader measured and reported it.
    pub latency: Option<i64>,
}

pub struct DataReader {
    pub id: String,
    data: RwLock<DataReaderUpdatableData>,
    pub connection: DataReaderConnection,
    has_first_init: AtomicBool,
    /// Namespace this connection works in. `SetNamespace` fixes it before the
    /// first subscription; a connection which never sends the packet stays in
    /// the default namespace, which is what every pre-namespace reader does.
    namespace: std::sync::RwLock<DbNamespaceName>,
}

impl DataReader {
    pub fn new(id: String, connection: DataReaderConnection) -> Self {
        Self {
            id,
            data: RwLock::new(DataReaderUpdatableData::new()),
            connection,
            has_first_init: AtomicBool::new(false),
            namespace: std::sync::RwLock::new(DbNamespaceName::default()),
        }
    }

    pub fn get_namespace(&self) -> DbNamespaceName {
        self.namespace.read().unwrap().clone()
    }

    /// Fixes the namespace of the connection. Refused once the connection is
    /// subscribed to something: the reader has already been initialized with
    /// that namespace's data, and swapping it underneath would leave it holding
    /// rows of one namespace while being told about updates of another.
    pub async fn set_namespace(&self, namespace: DbNamespaceName) -> Result<(), String> {
        // Re-stating the namespace it already has is not a change, so it stays
        // allowed after a subscription — that is what lets an HTTP reader
        // subscribe to a second table in the same namespace.
        if self.get_namespace() == namespace {
            return Ok(());
        }

        if self.data.read().await.has_any_subscription() {
            return Err(format!(
                "Namespace can not be changed to '{}' after the connection is subscribed to a table",
                namespace
            ));
        }

        *self.namespace.write().unwrap() = namespace;

        Ok(())
    }

    pub fn has_first_init(&self) -> bool {
        self.has_first_init.load(Ordering::Relaxed)
    }

    pub fn set_first_init(&self) {
        self.has_first_init.store(true, Ordering::SeqCst);
    }

    pub async fn has_table(&self, table_name: &str) -> bool {
        let read_access = self.data.read().await;
        read_access.has_table(table_name)
    }

    /*
       pub async fn set_name_as_reader(&self, name: String) {
           self.connection.set_name_as_reader(name).await;
       }

       pub async fn set_name_as_node(&self, location: String, version: String, compress_data: bool) {
           self.connection
               .set_name_as_node(location, version, compress_data)
               .await;
       }
    */
    pub fn get_name(&self) -> &str {
        self.connection.get_name()
    }

    pub async fn subscribe(&self, db_table: &Arc<DbTable>) {
        let mut write_access = self.data.write().await;
        write_access.subscribe(db_table);
    }

    pub async fn unsubscribe(&self, table_name: &str) {
        let mut write_access = self.data.write().await;
        write_access.unsubscribe(table_name);
    }

    fn get_ip(&self) -> String {
        match &self.connection {
            DataReaderConnection::Tcp(connection) => connection.get_ip(),
            DataReaderConnection::Http(connection) => connection.ip.to_string(),
        }
    }

    fn get_connected_moment(&self) -> DateTimeAsMicroseconds {
        match &self.connection {
            DataReaderConnection::Tcp(connection) => connection.connection_statistics().connected,
            DataReaderConnection::Http(connection) => connection.connected,
        }
    }

    pub fn get_last_incoming_moment(&self) -> DateTimeAsMicroseconds {
        match &self.connection {
            DataReaderConnection::Tcp(connection) => connection
                .connection_statistics()
                .last_receive_moment
                .as_date_time(),
            DataReaderConnection::Http(connection) => {
                connection.last_incoming_moment.as_date_time()
            }
        }
    }

    pub async fn get_metrics(&self) -> DataReadeMetrics {
        let session_id = self.id.to_string();
        let ip = self.get_ip();
        let connected = self.get_connected_moment();
        let last_incoming_moment = self.get_last_incoming_moment();

        let pending_to_send = self.get_pending_to_send();

        let latency = self.get_latency();

        let name = self.connection.get_name();

        let read_access = self.data.read().await;

        DataReadeMetrics {
            session_id,
            connected,
            last_incoming_moment,
            ip,
            name: name.to_string(),
            tables: read_access.get_table_names(),
            pending_to_send,
            latency,
        }
    }

    /// Keeps the round trip a TCP subscriber reported with its `PingWithLatency`.
    pub fn set_latency(&self, micros: u64) {
        if let DataReaderConnection::Tcp(connection) = &self.connection {
            connection.set_latency(micros);
        }
    }

    /// An HTTP reader does not ping over a connection of its own, so it never has one.
    pub fn get_latency(&self) -> Option<i64> {
        match &self.connection {
            DataReaderConnection::Tcp(connection) => connection.get_latency(),
            DataReaderConnection::Http(_) => None,
        }
    }

    pub fn is_node(&self) -> bool {
        match &self.connection {
            DataReaderConnection::Tcp(tcp_connection) => tcp_connection.is_node(),
            DataReaderConnection::Http(_) => false,
        }
    }

    pub fn get_pending_to_send(&self) -> usize {
        match &self.connection {
            DataReaderConnection::Tcp(connection) => connection.get_pending_to_send(),
            DataReaderConnection::Http(connection) => connection.get_pending_to_send(),
        }
    }

    pub async fn ping_http_servers(&self, now: DateTimeAsMicroseconds) {
        if let DataReaderConnection::Http(info) = &self.connection {
            info.ping(now).await;
        }
    }

    pub async fn get_sent_per_second(&self) -> Vec<usize> {
        match &self.connection {
            DataReaderConnection::Tcp(tcp) => tcp.sent_per_second.get_snapshot().await,
            DataReaderConnection::Http(_) => vec![],
        }
    }

    // Returns current (incoming, outgoing) bytes-per-second rates.
    // HTTP readers do not track traffic rate, so they report (0, 0).
    pub fn get_traffic_per_second(&self) -> (usize, usize) {
        match &self.connection {
            DataReaderConnection::Tcp(tcp) => {
                let statistics = tcp.connection_statistics();
                (
                    statistics.received_per_sec.get_value(),
                    statistics.sent_per_sec.get_value(),
                )
            }
            DataReaderConnection::Http(_) => (0, 0),
        }
    }
}
