use dioxus::prelude::*;

use crate::components::atoms::{StateTone, classify_reader};
use crate::models::ReaderApiModel;
use crate::settings::HealthThresholds;

#[component]
pub fn ReaderHealthGrid(readers: Vec<ReaderApiModel>) -> Element {
    let thresholds = *use_context::<Signal<HealthThresholds>>().read();
    let min_slots: usize = 100;
    let slots = readers.len().max(min_slots);

    let cells = (0..slots).map(|i| {
        if let Some(r) = readers.get(i) {
            let tone =
                classify_reader(&r.last_incoming_time, thresholds.warn_ms, thresholds.bad_ms);
            let cls = match tone {
                StateTone::Ok => "rh-cell rh-cell--ok",
                StateTone::Warn => "rh-cell rh-cell--warn",
                StateTone::Bad => "rh-cell rh-cell--bad",
                StateTone::Neutral => "rh-cell",
            };
            let tip = format!("{} · {}  ({})", r.id, r.name, r.last_incoming_time);
            rsx! {
                div { class: "{cls} has-tip",
                    span { class: "has-tip__tip", "{tip}" }
                }
            }
        } else {
            rsx! {
                div { class: "rh-cell" }
            }
        }
    });

    rsx! {
        div { class: "rh-grid", {cells} }
        div { class: "rh-legend",
            div { class: "rh-legend__item",
                span { class: "rh-legend__swatch", style: "background: var(--ok)" }
                span { "Healthy" }
            }
            div { class: "rh-legend__item",
                span { class: "rh-legend__swatch", style: "background: var(--warn)" }
                span { "Slow" }
            }
            div { class: "rh-legend__item",
                span { class: "rh-legend__swatch", style: "background: var(--danger)" }
                span { "Stalled" }
            }
            div { class: "rh-legend__item",
                span { class: "rh-legend__swatch", style: "background: var(--bg-sunken); border:1px solid var(--border)" }
                span { "Empty slot" }
            }
        }
    }
}
