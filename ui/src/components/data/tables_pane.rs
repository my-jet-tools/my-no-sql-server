use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::models::TableListItemApiModel;
use crate::utils::format_bytes;

/// What the tables list shows next to a table name. `/api/Tables/List` carries
/// no metrics at all, so both numbers come from the `/api/Status` poll the data
/// page already runs every 3 seconds — the same source the partition count has
/// always come from.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TableListMetrics {
    pub partitions_count: u64,
    pub data_size: u64,
}

#[component]
pub fn TablesPane(
    tables: Vec<TableListItemApiModel>,
    selected: String,
    writer_tables: HashSet<String>,
    metrics: HashMap<String, TableListMetrics>,
    on_select: EventHandler<String>,
) -> Element {
    let mut filter = use_signal(String::new);
    let filter_ra = filter.read();
    let needle = filter_ra.to_lowercase();
    let needle_empty = needle.is_empty();
    drop(filter_ra);

    let visible: Vec<TableListItemApiModel> = tables
        .into_iter()
        .filter(|t| needle_empty || t.name.to_lowercase().contains(&needle))
        .collect();

    let total = visible.len();

    // Summed over what is on screen, so filtering the list narrows the total too.
    // Skipped entirely until the status poll has answered — "0b" next to a full
    // list of tables reads as "the database is empty", which it is not.
    let header_count = if metrics.is_empty() {
        format!("{}", total)
    } else {
        let total_size: u64 = visible
            .iter()
            .filter_map(|t| metrics.get(&t.name))
            .map(|m| m.data_size)
            .sum();
        format!("{} · {}", total, format_bytes(total_size as f64))
    };

    let rows = visible.into_iter().map(|t| {
        let active = t.name == selected;
        let has_writer = writer_tables.contains(&t.name);
        let cls = if active {
            "tables-pane__item active"
        } else {
            "tables-pane__item"
        };
        let dot_cls = if has_writer {
            "tables-pane__dot has-writer"
        } else {
            "tables-pane__dot"
        };
        // A table with no entry is one the status poll has not covered yet, so
        // it gets a dash rather than a zero it would be indistinguishable from.
        let metric = metrics.get(&t.name);
        let part_str = match metric {
            Some(metric) => super::format_compact_count(metric.partitions_count),
            None => "—".to_string(),
        };
        let size_str = match metric {
            Some(metric) => format_bytes(metric.data_size as f64),
            None => "—".to_string(),
        };
        let name = t.name.clone();
        rsx! {
            div { class: cls, onclick: move |_| on_select.call(name.clone()),
                span { class: dot_cls }
                span { class: "tables-pane__name", "{t.name}" }
                span { class: "tables-pane__meta",
                    span {
                        class: "tables-pane__count",
                        title: "Partitions",
                        "{part_str}"
                    }
                    span {
                        class: "tables-pane__size",
                        title: "Data size",
                        "{size_str}"
                    }
                }
            }
        }
    });

    rsx! {
        aside { class: "tables-pane",
            div { class: "pane-header",
                span { class: "pane-header__title", "Tables" }
                span { class: "pane-header__count", "{header_count}" }
            }
            div { class: "pane-filter",
                input {
                    class: "filter-input",
                    placeholder: "filter tables…",
                    value: "{filter.read()}",
                    oninput: move |evt| filter.set(evt.value()),
                }
            }
            div { class: "pane-list", {rows} }
        }
    }
}
