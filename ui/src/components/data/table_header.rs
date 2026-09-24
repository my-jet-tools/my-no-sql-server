use dioxus::prelude::*;

use crate::components::atoms::{Badge, BadgeTone, Icon, IconKind};
use crate::models::TableApiModel;
use crate::utils::{format_bytes, format_unix_microseconds};

#[component]
pub fn TableHeader(
    name: String,
    stats: Option<TableApiModel>,
    on_refresh: EventHandler<()>,
    on_compression: EventHandler<()>,
) -> Element {
    // Only offered once the status poll has said which way the table is set —
    // clicking a guessed state would open the dialog on the wrong action.
    let compression = match stats.as_ref() {
        Some(t) => {
            let (text, tone) = if t.compressed {
                ("compressed", BadgeTone::Warn)
            } else {
                ("non compressed", BadgeTone::Neutral)
            };
            rsx! {
                button {
                    class: "table-header__badge-btn",
                    title: "Change in-memory compression",
                    onclick: move |_| on_compression.call(()),
                    Badge { text: text.to_string(), tone }
                }
            }
        }
        None => rsx! {},
    };

    let meta = if let Some(t) = stats {
        let size = format_bytes(t.data_size as f64);
        let persist_period = t
            .next_persist_time
            .filter(|v| *v > 0)
            .map(format_unix_microseconds)
            .unwrap_or_else(|| "—".to_string());
        let last_update = format_unix_microseconds(t.last_update_time);
        rsx! {
            div { class: "table-header__meta-item",
                "rows: " b { "{t.records_amount}" }
            }
            div { class: "table-header__meta-item",
                "partitions: " b { "{t.partitions_count}" }
            }
            div { class: "table-header__meta-item",
                "size: " b { "{size}" }
            }
            div { class: "table-header__meta-item",
                "next persist: " b { "{persist_period}" }
            }
            div { class: "table-header__meta-item",
                "updated: " b { "{last_update}" }
            }
        }
    } else {
        rsx! {
            div { class: "table-header__meta-item muted", "loading…" }
        }
    };

    rsx! {
        div { class: "table-header",
            div { class: "table-header__name",
                span { class: "table-header__title", "{name}" }
                {compression}
            }
            div { class: "table-header__meta", {meta} }
            div { class: "table-header__actions",
                button {
                    class: "topbar__icon-btn",
                    title: "Refresh",
                    onclick: move |_| on_refresh.call(()),
                    Icon { kind: IconKind::RefreshCw }
                }
                button { class: "topbar__icon-btn",
                    Icon { kind: IconKind::MoreHorizontal }
                }
            }
        }
    }
}
