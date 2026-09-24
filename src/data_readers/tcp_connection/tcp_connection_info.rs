use std::sync::{
    atomic::{AtomicI64, AtomicUsize, Ordering},
    Arc,
};

use my_no_sql_sdk::tcp_contracts::MyNoSqlTcpContract;
use my_tcp_sockets::tcp_connection::ConnectionStatistics;

use crate::tcp::MyNoSqlTcpConnection;

use super::SendPerSecond;

/// Kept in `latency_micros` until the subscriber reports its first round trip.
const LATENCY_NOT_MEASURED: i64 = -1;

pub enum ReaderName {
    AsReader(String),
    AsNode {
        location: String,
        #[allow(dead_code)]
        version: String,
    },
}

impl ReaderName {
    pub fn is_node(&self) -> bool {
        match self {
            ReaderName::AsReader(_) => false,
            ReaderName::AsNode { .. } => true,
        }
    }

    pub fn get_name(&self) -> &str {
        match self {
            ReaderName::AsReader(name) => name,
            ReaderName::AsNode { location, .. } => location,
        }
    }
}

pub struct TcpConnectionInfo {
    connection: Arc<MyNoSqlTcpConnection>,
    pub name: ReaderName,
    sent_per_second_accumulator: AtomicUsize,
    pub sent_per_second: SendPerSecond,
    pub compress_data: bool,
    /// Round trip of the connection in microseconds. Only the subscriber can measure it -
    /// it is the side which pings - so this is the number its last `PingWithLatency` carried.
    latency_micros: AtomicI64,
}

impl TcpConnectionInfo {
    pub fn new(
        connection: Arc<MyNoSqlTcpConnection>,
        name: ReaderName,
        compress_data: bool,
    ) -> Self {
        Self {
            connection,
            name,
            sent_per_second_accumulator: AtomicUsize::new(0),
            sent_per_second: SendPerSecond::new(),
            compress_data,
            latency_micros: AtomicI64::new(LATENCY_NOT_MEASURED),
        }
    }

    pub fn set_latency(&self, micros: u64) {
        let micros = i64::try_from(micros).unwrap_or(i64::MAX);
        self.latency_micros.store(micros, Ordering::Relaxed);
    }

    /// `None` until the subscriber reports a round trip - and for good, when its SDK
    /// predates `PingWithLatency` and it pings with a plain `Ping`.
    pub fn get_latency(&self) -> Option<i64> {
        let micros = self.latency_micros.load(Ordering::Relaxed);

        if micros < 0 {
            return None;
        }

        Some(micros)
    }

    pub fn connection_statistics(&self) -> &ConnectionStatistics {
        self.connection.statistics()
    }

    pub fn is_node(&self) -> bool {
        self.name.is_node()
    }

    pub fn get_id(&self) -> i32 {
        self.connection.id
    }

    pub fn get_ip(&self) -> String {
        match &self.connection.addr {
            Some(addr) => format!("{}", addr),
            None => "unknown".to_string(),
        }
    }

    pub fn is_compressed_data(&self) -> bool {
        self.compress_data
    }

    pub fn get_name(&self) -> &str {
        self.name.get_name()
    }

    /*
    pub async fn set_name_as_reader(&self, name: String) {
        let mut write_access = self.name.lock().await;
        *write_access = ReaderName::AsReader(name);
    }

    pub async fn set_name_as_node(&self, location: String, version: String) {
        self.is_node.store(true, Ordering::SeqCst);
        let mut write_access = self.name.lock().await;
        *write_access = ReaderName::AsNode { location, version };
    }
     */

    pub async fn send(&self, tcp_contract: &[MyNoSqlTcpContract]) {
        let sent_amount = self.connection.send_many(tcp_contract);
        self.sent_per_second_accumulator
            .fetch_add(sent_amount, std::sync::atomic::Ordering::SeqCst);
    }

    pub async fn timer_1sec_tick(&self) {
        let value = self
            .sent_per_second_accumulator
            .swap(0, std::sync::atomic::Ordering::SeqCst);
        self.sent_per_second.add(value).await;
    }

    pub fn get_pending_to_send(&self) -> usize {
        self.connection
            .statistics()
            .pending_to_send_buffer_size
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}
