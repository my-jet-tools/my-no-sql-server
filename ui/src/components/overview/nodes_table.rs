use dioxus::prelude::*;

use crate::components::atoms::{
    Badge, BadgeTone, LatencyPill, Sparkline, StatusDot, classify_reader,
};
use crate::models::ReaderApiModel;
use crate::settings::HealthThresholds;

/// Tables shown per node before the rest collapse into a "+N" badge.
const VISIBLE_TABLES: usize = 3;

/// MyNoSqlServer nodes replicating from this server. A node connects as a TCP
/// reader greeted with `GreetingFromNode`; its name is the node's location.
///
/// A node opens a connection of its own per namespace it replicates, all under
/// the same location — so a row is a connection, and one node can own several.
#[component]
pub fn NodesTable(nodes: Vec<ReaderApiModel>) -> Element {
    let thresholds = *use_context::<Signal<HealthThresholds>>().read();

    if nodes.is_empty() {
        return rsx! {
            div { class: "card",
                div { class: "card__header",
                    span { class: "card__title", "Nodes" }
                    span { class: "card__subtitle", "0 connected" }
                }
                div { class: "card__body",
                    div { style: "color:var(--text-muted); font-size:12px; text-align:center; padding:14px;",
                        "No nodes connected"
                    }
                }
            }
        };
    }

    // /api/Status lists readers in no particular order; sorted, the rows stay put
    // between the once-a-second refreshes.
    let mut nodes = nodes;
    nodes.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.namespace.cmp(&b.namespace))
            .then_with(|| a.id.cmp(&b.id))
    });

    let node_count = count_distinct_nodes(&nodes);
    let connection_count = nodes.len();

    let rows = nodes.into_iter().map(|node| {
        let tone = classify_reader(&node.last_incoming_time, thresholds.warn_ms, thresholds.bad_ms);

        let table_badges = node.tables.iter().take(VISIBLE_TABLES).cloned().map(|t| {
            rsx! {
                Badge { text: t, tone: BadgeTone::Reader }
            }
        });

        let overflow_badge = if node.tables.len() > VISIBLE_TABLES {
            rsx! {
                Badge {
                    text: format!("+{}", node.tables.len() - VISIBLE_TABLES),
                    tone: BadgeTone::Neutral,
                }
            }
        } else {
            rsx! {}
        };

        rsx! {
            tr {
                td {
                    div { style: "display:flex; align-items:center; gap:8px;",
                        StatusDot { tone }
                        span { "{node.name}" }
                    }
                }
                td { class: "mono", "{node.namespace}" }
                td { class: "mono muted",
                    span { class: "dt-ellipsis", "{node.ip}" }
                }
                td {
                    LatencyPill { latency: node.latency }
                }
                td { style: "max-width:220px;",
                    Sparkline { values: node.sent_per_second, bytes_label: true }
                }
                td {
                    span { class: "badge-list",
                        {table_badges}
                        {overflow_badge}
                    }
                }
                td { class: "mono muted", "{node.last_incoming_time}" }
            }
        }
    });

    rsx! {
        div { class: "card",
            div { class: "card__header",
                span { class: "card__title", "Nodes" }
                span { class: "card__subtitle", "{node_count} connected · {connection_count} connections" }
            }
            table { class: "dt",
                thead {
                    tr {
                        th { "Node" }
                        th { "Namespace" }
                        th { "Address" }
                        th { "Latency" }
                        th { "Traffic" }
                        th { "Tables" }
                        th { "Last incoming" }
                    }
                }
                tbody { {rows} }
            }
        }
    }
}

/// Nodes, not connections: the connections a node opens per namespace all carry
/// its location as the name.
pub fn count_distinct_nodes(nodes: &[ReaderApiModel]) -> usize {
    nodes
        .iter()
        .map(|node| node.name.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}
