use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};

use my_tcp_sockets::ThreadsStatistics;
use prometheus::{Encoder, IntGauge, IntGaugeVec, Opts, Registry, TextEncoder};

use crate::operations::DbTableMetrics;

pub trait UpdatePendingToSyncModel {
    fn get_name(&self) -> &str;
    fn get_pending_to_sync(&self) -> usize;
}

/// Labels of one `reader_latency_microseconds` series: namespace, reader name
/// and kind ("node" or "reader").
pub type ReaderLatencyKey = (String, String, &'static str);

pub struct PrometheusMetrics {
    registry: Registry,
    partitions_amount: IntGaugeVec,
    table_size: IntGaugeVec,
    persist_amount: IntGaugeVec,
    tcp_connections: IntGaugeVec,
    unix_connections: IntGaugeVec,
    tcp_connections_changes: IntGaugeVec,
    http_connections_count: IntGauge,
    persist_delay_in_seconds: IntGaugeVec,
    pending_to_sync: IntGaugeVec,
    reader_latency: IntGaugeVec,
    /// Series `reader_latency` holds right now - the ones missing from the next
    /// update belong to readers which are gone, and are removed.
    reader_latency_keys: Mutex<BTreeSet<ReaderLatencyKey>>,
}

const TABLE_NAME: &str = "table_name";
const READER: &str = "reader";
/// "node" for another MyNoSqlServer node replicating from this one, "reader" otherwise.
const KIND: &str = "kind";
/// Namespace of the table the metric belongs to. Always present — the default
/// namespace reports itself as "default" rather than as an absent label.
const NAMESPACE: &str = "ns";
const TCP_METRIC: &str = "tcp_metric";

impl PrometheusMetrics {
    pub fn new() -> Self {
        let registry = Registry::new();
        let partitions_amount = create_partitions_amount_gauge();
        let table_size = create_table_size_gauge();
        let persist_amount = create_persist_amount_gauge();
        let tcp_connections = create_tcp_connections();
        let unix_connections = create_unix_connections();
        let tcp_connections_changes = create_tcp_connections_changes();
        let fatal_errors_count = create_fatal_errors_count();

        let pending_to_sync = create_pending_to_sync();

        let reader_latency = create_reader_latency();

        let persist_delay_in_seconds = create_persist_delay_in_seconds();

        let http_connections_count = create_http_connections_count();

        registry
            .register(Box::new(http_connections_count.clone()))
            .unwrap();

        registry
            .register(Box::new(partitions_amount.clone()))
            .unwrap();

        registry.register(Box::new(table_size.clone())).unwrap();
        registry.register(Box::new(persist_amount.clone())).unwrap();
        registry
            .register(Box::new(fatal_errors_count.clone()))
            .unwrap();

        registry
            .register(Box::new(tcp_connections.clone()))
            .unwrap();

        registry
            .register(Box::new(tcp_connections_changes.clone()))
            .unwrap();

        registry
            .register(Box::new(persist_delay_in_seconds.clone()))
            .unwrap();

        registry
            .register(Box::new(pending_to_sync.clone()))
            .unwrap();

        registry
            .register(Box::new(reader_latency.clone()))
            .unwrap();

        return Self {
            registry,
            partitions_amount,
            table_size,
            persist_amount,
            tcp_connections,
            tcp_connections_changes,
            persist_delay_in_seconds,
            pending_to_sync,
            http_connections_count,
            unix_connections,
            reader_latency,
            reader_latency_keys: Mutex::new(BTreeSet::new()),
        };
    }

    /// Replaces the whole set of latency series with `latencies` - every reader
    /// which reported one, keyed by its labels. A series is dropped once its
    /// reader stops being in the set, which is how a disconnect reaches it.
    pub fn update_readers_latency(&self, latencies: BTreeMap<ReaderLatencyKey, i64>) {
        let mut keys = self.reader_latency_keys.lock().unwrap();

        for key in keys.iter() {
            if !latencies.contains_key(key) {
                let (namespace, reader, kind) = key;
                let _ = self
                    .reader_latency
                    .remove_label_values(&[namespace.as_str(), reader.as_str(), *kind]);
            }
        }

        for ((namespace, reader, kind), latency) in latencies.iter() {
            self.reader_latency
                .with_label_values(&[namespace.as_str(), reader.as_str(), *kind])
                .set(*latency);
        }

        *keys = latencies.into_keys().collect();
    }

    pub fn update_table_metrics(
        &self,
        namespace: &str,
        table_name: &str,
        table_metrics: &DbTableMetrics,
        http_connections_count: i64,
    ) {
        let partitions_amount_value = table_metrics.partitions_amount as i64;
        self.partitions_amount
            .with_label_values(&[namespace, table_name])
            .set(partitions_amount_value);

        let table_size_value = table_metrics.table_size as i64;
        self.table_size
            .with_label_values(&[namespace, table_name])
            .set(table_size_value);

        let persist_amount_value = table_metrics.persist_amount as i64;
        self.persist_amount
            .with_label_values(&[namespace, table_name])
            .set(persist_amount_value);

        self.http_connections_count.set(http_connections_count);
    }

    pub fn update_persist_delay(&self, namespace: &str, table_name: &str, persist_delay: i64) {
        self.persist_delay_in_seconds
            .with_label_values(&[namespace, table_name])
            .set(persist_delay);
    }

    pub fn get_http_connections_amount(&self) -> i64 {
        self.http_connections_count.get()
    }

    pub fn update_pending_to_sync<TUpdatePendingToSyncModel: UpdatePendingToSyncModel>(
        &self,
        data_reader_connection: &TUpdatePendingToSyncModel,
    ) {
        let name = data_reader_connection.get_name();

        let pending_to_sync = data_reader_connection.get_pending_to_sync();

        self.pending_to_sync
            .with_label_values(&[&name])
            .set(pending_to_sync as i64);
    }

    pub fn remove_pending_to_sync<TUpdatePendingToSyncModel: UpdatePendingToSyncModel>(
        &self,
        data_reader_connection: &TUpdatePendingToSyncModel,
    ) {
        let name = data_reader_connection.get_name();

        let result = self.pending_to_sync.remove_label_values(&[&name]);

        if let Err(err) = result {
            println!(
                "Can not remove pending to sync metric for data reader {}: {:?}",
                name, err
            );
        }
    }
    pub fn mark_new_tcp_connection(&self) {
        self.tcp_connections.with_label_values(&["count"]).inc();
        self.tcp_connections_changes
            .with_label_values(&["connected"])
            .inc();
    }

    pub fn update_tcp_threads(&self, threads_statistics: &ThreadsStatistics) {
        self.tcp_connections
            .with_label_values(&["ping_threads"])
            .set(threads_statistics.ping_threads.get());

        self.tcp_connections
            .with_label_values(&["read_threads"])
            .set(threads_statistics.read_threads.get());

        self.tcp_connections
            .with_label_values(&["connection_objects"])
            .set(threads_statistics.connections_objects.get());
    }

    pub fn update_unix_threads(&self, threads_statistics: &ThreadsStatistics) {
        self.unix_connections
            .with_label_values(&["ping_threads"])
            .set(threads_statistics.ping_threads.get());

        self.unix_connections
            .with_label_values(&["read_threads"])
            .set(threads_statistics.read_threads.get());

        self.unix_connections
            .with_label_values(&["connection_objects"])
            .set(threads_statistics.connections_objects.get());
    }

    pub fn mark_new_tcp_disconnection(&self) {
        self.tcp_connections.with_label_values(&["count"]).dec();
        self.tcp_connections_changes
            .with_label_values(&["disconnected"])
            .inc();
    }

    pub fn build(&self) -> String {
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        return String::from_utf8(buffer).unwrap();
    }
}

fn create_partitions_amount_gauge() -> IntGaugeVec {
    let gauge_opts = Opts::new(
        format!("table_partitions_amount"),
        format!("table partitions amount"),
    );

    let labels = &[NAMESPACE, TABLE_NAME];
    IntGaugeVec::new(gauge_opts, labels).unwrap()
}

fn create_table_size_gauge() -> IntGaugeVec {
    let gauge_opts = Opts::new(format!("table_size"), format!("table size"));

    let labels = &[NAMESPACE, TABLE_NAME];
    IntGaugeVec::new(gauge_opts, labels).unwrap()
}

fn create_persist_amount_gauge() -> IntGaugeVec {
    let gauge_opts = Opts::new(format!("persist_amount"), format!("persist amount"));

    let labels = &[NAMESPACE, TABLE_NAME];
    IntGaugeVec::new(gauge_opts, labels).unwrap()
}

fn create_pending_to_sync() -> IntGaugeVec {
    let gauge_opts = Opts::new(
        format!("pending_to_send"),
        format!("pending bytes to send to reader"),
    );

    // Labelled by the reader name, not by a table — this one stays single-label.
    let labels = &[TABLE_NAME];
    IntGaugeVec::new(gauge_opts, labels).unwrap()
}

fn create_reader_latency() -> IntGaugeVec {
    let gauge_opts = Opts::new(
        "reader_latency_microseconds",
        "Round trip to the reader, as it measured it and reported with PingWithLatency. The worst one when several connections share the labels",
    );

    let labels = &[NAMESPACE, READER, KIND];
    IntGaugeVec::new(gauge_opts, labels).unwrap()
}

fn create_fatal_errors_count() -> IntGauge {
    IntGauge::new("fatal_errors_count", "Fatal errors count").unwrap()
}

fn create_http_connections_count() -> IntGauge {
    IntGauge::new("http_connections_count", "Http connections count").unwrap()
}

fn create_persist_delay_in_seconds() -> IntGaugeVec {
    let gauge_opts = Opts::new(
        format!("persist_delay_sec"),
        format!("Current delay of persistence operation in seconds"),
    );

    let labels = &[NAMESPACE, TABLE_NAME];
    IntGaugeVec::new(gauge_opts, labels).unwrap()
}

fn create_tcp_connections_changes() -> IntGaugeVec {
    let gauge_opts = Opts::new(format!("tcp_changes_count"), format!("Tcp Changes Count"));

    let labels = &[TCP_METRIC];
    IntGaugeVec::new(gauge_opts, labels).unwrap()
}

fn create_tcp_connections() -> IntGaugeVec {
    let gauge_opts = Opts::new(format!("tcp_connections"), format!("Tcp Connections"));
    let labels = &[TCP_METRIC];
    IntGaugeVec::new(gauge_opts, labels).unwrap()
}

fn create_unix_connections() -> IntGaugeVec {
    let gauge_opts = Opts::new(
        format!("unix_connections"),
        format!("Unix socket connections"),
    );
    let labels = &[TCP_METRIC];
    IntGaugeVec::new(gauge_opts, labels).unwrap()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::PrometheusMetrics;

    fn latency_of(metrics: &PrometheusMetrics, reader: &str) -> Option<String> {
        let reader_label = format!("reader=\"{}\"", reader);

        metrics
            .build()
            .lines()
            .filter(|line| line.starts_with("reader_latency_microseconds{"))
            .find(|line| line.contains(reader_label.as_str()))
            .map(|line| line.rsplit(' ').next().unwrap().to_string())
    }

    #[test]
    fn latency_of_a_reader_which_is_gone_is_removed() {
        let metrics = PrometheusMetrics::new();

        let mut latencies = BTreeMap::new();
        latencies.insert(("default".to_string(), "node-a".to_string(), "node"), 1_500);
        latencies.insert(("default".to_string(), "app".to_string(), "reader"), 700);
        metrics.update_readers_latency(latencies);

        assert_eq!(Some("1500".to_string()), latency_of(&metrics, "node-a"));
        assert_eq!(Some("700".to_string()), latency_of(&metrics, "app"));

        let mut latencies = BTreeMap::new();
        latencies.insert(("default".to_string(), "app".to_string(), "reader"), 900);
        metrics.update_readers_latency(latencies);

        assert_eq!(None, latency_of(&metrics, "node-a"));
        assert_eq!(Some("900".to_string()), latency_of(&metrics, "app"));
    }
}
