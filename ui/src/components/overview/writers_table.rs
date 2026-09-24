use dioxus::prelude::*;

use crate::components::atoms::{Badge, BadgeTone, StatePill, StateTone, classify_reader};
use crate::models::WriterApiModel;
use crate::settings::HealthThresholds;

#[component]
pub fn WritersTable(writers: Vec<WriterApiModel>) -> Element {
    let thresholds = *use_context::<Signal<HealthThresholds>>().read();
    if writers.is_empty() {
        return rsx! {
            div { class: "card",
                div { class: "card__header",
                    span { class: "card__title", "Writers" }
                    span { class: "card__subtitle", "0 connected" }
                }
                div { class: "card__body",
                    div { style: "color:var(--text-muted); font-size:12px; text-align:center; padding:14px;",
                        "No writers connected"
                    }
                }
            }
        };
    }

    let count = writers.len();
    let rows = writers.into_iter().map(|w| {
        let tables = w.tables.iter().cloned().map(|t| {
            rsx! {
                Badge { text: t, tone: BadgeTone::Writer }
            }
        });
        let tone = classify_reader(&w.last_update, thresholds.warn_ms, thresholds.bad_ms);
        let state_label = match tone {
            StateTone::Ok => "live",
            StateTone::Warn => "lagging",
            StateTone::Bad => "stalled",
            StateTone::Neutral => "—",
        };

        rsx! {
            tr {
                td { "{w.name}" }
                td { class: "mono muted", "{w.version}" }
                td {
                    span { class: "badge-list", {tables} }
                }
                td { class: "mono", "{w.last_update}" }
                td {
                    StatePill { label: state_label.to_string(), tone }
                }
            }
        }
    });

    rsx! {
        div { class: "card",
            div { class: "card__header",
                span { class: "card__title", "Writers" }
                span { class: "card__subtitle", "{count} connected" }
            }
            table { class: "dt",
                thead {
                    tr {
                        th { "App" }
                        th { "Version" }
                        th { "Tables" }
                        th { "Last ping" }
                        th { "State" }
                    }
                }
                tbody { {rows} }
            }
        }
    }
}
