use dioxus::prelude::*;
use std::collections::HashMap;

use crate::models::PartitionMetricApiModel;

#[component]
pub fn PartitionsPane(
    partitions: Vec<String>,
    metrics: HashMap<String, PartitionMetricApiModel>,
    selected: Option<String>,
    on_select: EventHandler<String>,
) -> Element {
    let mut filter = use_signal(String::new);
    let filter_ra = filter.read();
    let needle = filter_ra.to_lowercase();
    let needle_empty = needle.is_empty();
    drop(filter_ra);

    let visible: Vec<String> = partitions
        .into_iter()
        .filter(|p| needle_empty || p.to_lowercase().contains(&needle))
        .collect();

    let total = visible.len();

    let rows = visible.into_iter().map(|pk| {
        let active = selected.as_ref() == Some(&pk);
        let cls = if active {
            "partitions-pane__item active"
        } else {
            "partitions-pane__item"
        };
        let metric = metrics.get(&pk);
        let records = metric.map(|m| m.records_count).unwrap_or(0);
        let size = metric.map(|m| m.data_size).unwrap_or(0);
        let records_str = super::format_compact_count(records);
        let size_str = crate::utils::format_bytes(size as f64);
        let pk_for_handler = pk.clone();
        rsx! {
            div { class: cls, onclick: move |_| on_select.call(pk_for_handler.clone()),
                span { class: "partitions-pane__name", "{pk}" }
                span { class: "partitions-pane__meta",
                    span {
                        class: "partitions-pane__count",
                        title: "Records",
                        "{records_str}"
                    }
                    span {
                        class: "partitions-pane__size",
                        title: "Size in bytes",
                        "{size_str}"
                    }
                }
            }
        }
    });

    rsx! {
        aside { class: "partitions-pane",
            div { class: "pane-header",
                span { class: "pane-header__title", "Partitions · {total}" }
            }
            div { class: "pane-filter",
                input {
                    class: "filter-input",
                    placeholder: "filter partitions…",
                    value: "{filter.read()}",
                    oninput: move |evt| filter.set(evt.value()),
                }
            }
            div { class: "pane-list", {rows} }
        }
    }
}
