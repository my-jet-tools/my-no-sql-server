use std::{collections::BTreeMap, sync::Arc};

use my_http_server::HttpConnectionsCounter;
use my_no_sql_sdk::core::rust_extensions::{MyTimerTick, RepeatTimerIteration};
use my_tcp_sockets::ThreadsStatistics;

use crate::app::AppContext;

pub struct MetricsUpdater {
    app: Arc<AppContext>,
    http_connections_count: HttpConnectionsCounter,
    tcp_threads_statistics: Arc<ThreadsStatistics>,
    unix_threads_statistics: Option<Arc<ThreadsStatistics>>,
}

impl MetricsUpdater {
    pub fn new(
        app: Arc<AppContext>,
        http_connections_count: HttpConnectionsCounter,
        tcp_threads_statistics: Arc<ThreadsStatistics>,
        unix_threads_statistics: Option<Arc<ThreadsStatistics>>,
    ) -> Self {
        Self {
            app,
            http_connections_count,
            tcp_threads_statistics,
            unix_threads_statistics,
        }
    }
}

#[async_trait::async_trait]
impl MyTimerTick for MetricsUpdater {
    async fn tick(&self) -> RepeatTimerIteration {
        self.app
            .metrics
            .update_tcp_threads(&self.tcp_threads_statistics);

        if let Some(unix_threads_statistics) = self.unix_threads_statistics.as_ref() {
            self.app
                .metrics
                .update_unix_threads(unix_threads_statistics.as_ref());
        }

        for db_namespace in self.app.namespaces.get_all() {
            let tables = db_namespace.db.get_tables();
            for db_table in tables.iter() {
                let table_metrics =
                    crate::operations::get_table_metrics(&db_namespace, db_table.as_ref()).await;

                let persist_delay = if let Some(last_persist_time) = table_metrics.last_persist_time
                {
                    if last_persist_time.unix_microseconds
                        < table_metrics.last_update_time.unix_microseconds
                    {
                        let duration = table_metrics
                            .last_update_time
                            .duration_since(last_persist_time)
                            .as_positive_or_zero();

                        duration.as_secs() as i64
                    } else {
                        0
                    }
                } else {
                    0
                };

                let http_connections_amount = self.http_connections_count.get_connections_amount();

                self.app.metrics.update_table_metrics(
                    db_namespace.name.as_str(),
                    db_table.name.as_str(),
                    &table_metrics,
                    http_connections_amount,
                );

                self.app.metrics.update_persist_delay(
                    db_namespace.name.as_str(),
                    db_table.name.as_str(),
                    persist_delay,
                );
            }
        }

        self.app.write_payloads_per_second.one_second_tick();
        self.app.write_bytes_per_second.one_second_tick();
        self.app.writers_traffic.one_second_tick();

        let mut readers_latency = BTreeMap::new();

        for reader in self.app.data_readers.get_all().await {
            self.app.metrics.update_pending_to_sync(&reader.connection);

            if let Some(latency) = reader.get_latency() {
                let kind = if reader.is_node() { "node" } else { "reader" };
                let key = (
                    reader.get_namespace().to_string(),
                    reader.get_name().to_string(),
                    kind,
                );

                // Replicas of one app connect under one name - the slowest of them is
                // the one worth an alert.
                let worst = readers_latency.entry(key).or_insert(latency);
                *worst = (*worst).max(latency);
            }

            reader.connection.one_sec_tick().await;
        }

        self.app.metrics.update_readers_latency(readers_latency);

        RepeatTimerIteration::WithInterval
    }
}
