use std::time::Duration;

use dioxus::prelude::*;

use crate::api::get_connections;
use crate::components::atoms::{Badge, BadgeTone, LatencyPill, MiniChart, MiniChartSeries};
use crate::models::DEFAULT_NAMESPACE;
use crate::models::{ConnectionReaderApiModel, ConnectionWriterApiModel, ConnectionsApiModel};
use crate::utils::{format_bytes, format_bytes_per_sec};

const MAX_POINTS: usize = 120;

#[derive(Clone, Copy)]
struct Sample {
    incoming: u64,
    outgoing: u64,
    payloads: u64,
    write_bytes: u64,
}

#[derive(Default)]
struct ConnectionsState {
    started: bool,
    snapshot: Option<ConnectionsApiModel>,
    history: Vec<Sample>,
}

impl ConnectionsState {
    fn push(&mut self, snapshot: ConnectionsApiModel) {
        self.history.push(Sample {
            incoming: snapshot.incoming_per_second,
            outgoing: snapshot.outgoing_per_second,
            payloads: snapshot.write_payloads_per_second,
            write_bytes: snapshot.write_bytes_per_second,
        });
        if self.history.len() > MAX_POINTS {
            let overflow = self.history.len() - MAX_POINTS;
            self.history.drain(0..overflow);
        }
        self.snapshot = Some(snapshot);
    }
}

#[component]
pub fn Connections() -> Element {
    let mut cs = use_signal(ConnectionsState::default);

    let started_val = cs.read().started;
    let on_mount = move |_| {
        if started_val {
            return;
        }
        cs.write().started = true;
        spawn(async move {
            loop {
                match get_connections().await {
                    Ok(result) => cs.write().push(result),
                    Err(err) => {
                        dioxus_utils::console_log(&format!("Connections error: {}", err));
                    }
                }
                dioxus_utils::js::sleep(Duration::from_secs(1)).await;
            }
        });
    };

    let cs_ra = cs.read();
    let history = cs_ra.history.clone();
    let snapshot = cs_ra.snapshot.clone();
    drop(cs_ra);

    let content = match snapshot {
        Some(snapshot) => render_connections(&history, &snapshot),
        None => rsx! {
            div { class: "empty-state",
                div { class: "empty-state__title", "Connecting to server…" }
            }
        },
    };

    rsx! {
        section { class: "page page--padded", onmounted: on_mount,
            div { class: "connections", {content} }
        }
    }
}

fn render_connections(history: &[Sample], snapshot: &ConnectionsApiModel) -> Element {
    // The UI works in one namespace at a time, so the connection lists show that
    // namespace only. The traffic charts above stay server-wide: those counters
    // are process-level and can not be attributed to a namespace at all.
    let namespace =
        crate::storage::load_namespace().unwrap_or_else(|| DEFAULT_NAMESPACE.to_string());

    let readers: Vec<ConnectionReaderApiModel> = snapshot
        .readers
        .iter()
        .filter(|itm| itm.namespace == namespace)
        .cloned()
        .collect();

    let writers: Vec<ConnectionWriterApiModel> = snapshot
        .writers
        .iter()
        .filter(|itm| itm.namespace == namespace)
        .cloned()
        .collect();

    let incoming = snapshot.incoming_per_second;
    let outgoing = snapshot.outgoing_per_second;

    let traffic_series = vec![
        MiniChartSeries::new(
            history.iter().map(|s| s.incoming as f64).collect(),
            "mini-chart__line--in",
        ),
        MiniChartSeries::new(
            history.iter().map(|s| s.outgoing as f64).collect(),
            "mini-chart__line--out",
        ),
    ];
    let traffic_max = history
        .iter()
        .map(|s| s.incoming.max(s.outgoing))
        .max()
        .unwrap_or(0)
        .max(1) as f64;

    let payloads = snapshot.write_payloads_per_second;
    let write_bytes = snapshot.write_bytes_per_second;

    let payloads_series = vec![MiniChartSeries::new(
        history.iter().map(|s| s.payloads as f64).collect(),
        "mini-chart__line--write",
    )];
    let payloads_max = history.iter().map(|s| s.payloads).max().unwrap_or(0).max(1) as f64;

    let throughput_series = vec![MiniChartSeries::new(
        history.iter().map(|s| s.write_bytes as f64).collect(),
        "mini-chart__line--write",
    )];
    let throughput_max = history
        .iter()
        .map(|s| s.write_bytes)
        .max()
        .unwrap_or(0)
        .max(1) as f64;

    rsx! {
        div { class: "card",
            div { class: "card__header",
                span { class: "card__title", "Reader traffic · all readers" }
                div { class: "conn-legend",
                    span { class: "conn-legend__item",
                        span { class: "conn-legend__dot conn-legend__dot--in" }
                        "Incoming "
                        b { "{format_bytes_per_sec(incoming as f64)}" }
                    }
                    span { class: "conn-legend__item",
                        span { class: "conn-legend__dot conn-legend__dot--out" }
                        "Outgoing "
                        b { "{format_bytes_per_sec(outgoing as f64)}" }
                    }
                }
            }
            div { class: "card__body",
                MiniChart {
                    series: traffic_series,
                    max: traffic_max,
                    label: format_bytes_per_sec(traffic_max),
                }
            }
        }

        div { class: "card",
            div { class: "card__header",
                span { class: "card__title", "Writer traffic · all writers" }
                div { class: "conn-legend",
                    span { class: "conn-legend__item",
                        span { class: "conn-legend__dot conn-legend__dot--write" }
                        "Throughput "
                        b { "{format_bytes_per_sec(write_bytes as f64)}" }
                    }
                    span { class: "conn-legend__item",
                        span { class: "conn-legend__dot conn-legend__dot--write" }
                        "Payloads "
                        b { "{payloads} req/s" }
                    }
                }
            }
            div { class: "card__body",
                div { class: "conn-chart-label", "Throughput" }
                MiniChart {
                    series: throughput_series,
                    max: throughput_max,
                    label: format_bytes_per_sec(throughput_max),
                    height: 140.0,
                }
                div { class: "conn-chart-label", "Payloads (req/s)" }
                MiniChart {
                    series: payloads_series,
                    max: payloads_max,
                    label: format!("{} req/s", payloads_max as usize),
                    height: 140.0,
                }
            }
        }

        {render_readers_table(&readers, namespace.as_str())}
        {render_writers_table(&writers, namespace.as_str())}
    }
}

fn render_readers_table(readers: &[ConnectionReaderApiModel], namespace: &str) -> Element {
    if readers.is_empty() {
        return rsx! {
            div { class: "card",
                div { class: "card__body",
                    div { class: "empty-state",
                        div { class: "empty-state__title", "No active connections in namespace {namespace}" }
                    }
                }
            }
        };
    }

    let rows = readers.iter().map(|reader| {
        let kind = if reader.is_node { "node" } else { "reader" };
        rsx! {
            tr {
                td { class: "conn-table__id", "{reader.id}" }
                td { "{reader.name}" }
                td { "{reader.ip}" }
                td { class: "conn-table__kind", "{kind}" }
                td {
                    LatencyPill { latency: reader.latency }
                }
                td { class: "conn-table__num", "{format_bytes_per_sec(reader.incoming_per_second as f64)}" }
                td { class: "conn-table__num", "{format_bytes_per_sec(reader.outgoing_per_second as f64)}" }
                td { class: "conn-table__num", "{format_bytes(reader.pending_to_send as f64)}" }
            }
        }
    });

    rsx! {
        div { class: "card",
            div { class: "card__header",
                span { class: "card__title", "Readers" }
                span { class: "card__subtitle", "{readers.len()} connected · ns {namespace}" }
            }
            div { class: "card__body",
                table { class: "conn-table",
                    thead {
                        tr {
                            th { "ID" }
                            th { "Name" }
                            th { "IP" }
                            th { "Kind" }
                            th { "Latency" }
                            th { class: "conn-table__num", "Incoming" }
                            th { class: "conn-table__num", "Outgoing" }
                            th { class: "conn-table__num", "Pending" }
                        }
                    }
                    tbody { {rows} }
                }
            }
        }
    }
}

fn render_writers_table(writers: &[ConnectionWriterApiModel], namespace: &str) -> Element {
    if writers.is_empty() {
        return rsx! {
            div { class: "card",
                div { class: "card__header",
                    span { class: "card__title", "Writers" }
                    span { class: "card__subtitle", "0 connected · ns {namespace}" }
                }
                div { class: "card__body",
                    div { class: "empty-state",
                        div { class: "empty-state__title", "No active writers in namespace {namespace}" }
                    }
                }
            }
        };
    }

    let mut writers = writers.to_vec();
    writers.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.session.cmp(&b.session)));

    let rows = writers.iter().map(|writer| {
        let table_badges = writer.tables.iter().cloned().map(|t| {
            rsx! {
                Badge { text: t, tone: BadgeTone::Writer }
            }
        });
        let rate_per_second = format_bytes_per_sec(writer.bytes_per_second as f64);
        rsx! {
            tr {
                td { "{writer.name}" }
                td { class: "conn-table__id", "{writer.session}" }
                td { class: "conn-table__id", "{writer.version}" }
                td { class: "conn-table__id", "{writer.addr}" }
                td { class: "conn-table__num", "{writer.tables.len()}" }
                td { span { class: "badge-list", {table_badges} } }
                td { class: "conn-table__num", "{writer.req_per_second} req/s" }
                td { class: "conn-table__num", "{rate_per_second}" }
                td { class: "conn-table__num", "{writer.last_incoming_time}" }
            }
        }
    });

    rsx! {
        div { class: "card",
            div { class: "card__header",
                span { class: "card__title", "Writers" }
                span { class: "card__subtitle", "{writers.len()} connected · ns {namespace}" }
            }
            div { class: "card__body",
                table { class: "conn-table",
                    thead {
                        tr {
                            th { "Name" }
                            th { "Session" }
                            th { "Version" }
                            th { "Addr" }
                            th { class: "conn-table__num", "Tables" }
                            th { "Table list" }
                            th { class: "conn-table__num", "Req/s" }
                            th { class: "conn-table__num", "Rate" }
                            th { class: "conn-table__num", "Last ping" }
                        }
                    }
                    tbody { {rows} }
                }
            }
        }
    }
}
