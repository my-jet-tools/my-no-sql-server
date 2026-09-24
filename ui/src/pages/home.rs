use std::time::Duration;

use dioxus::prelude::*;

use crate::AppContext;
use crate::api::get_status;
use crate::components::atoms::{StateTone, classify_reader};
use crate::components::overview::{
    HealthBanner, HealthTone, NodesTable, ReaderHealthGrid, ReadersTable, StatsRow, TableCoverage,
    WritersTable,
};
use crate::models::{InitializedApiModel, ReaderApiModel, StatusApiModel};
use crate::settings::HealthThresholds;

#[component]
pub fn Home() -> Element {
    let mut data = use_signal(|| None::<StatusApiModel>);
    let mut started = use_signal(|| false);
    let tick = use_signal(|| 0u64);
    let app_ctx = use_context::<Signal<AppContext>>();

    let started_val = *started.read();
    let on_mount = move |_| {
        if started_val {
            return;
        }
        *started.write() = true;
        let mut ctx = app_ctx;
        spawn(async move {
            loop {
                match get_status().await {
                    Ok(result) => {
                        ctx.write().status = Some(result.clone());
                        data.set(Some(result));
                    }
                    Err(err) => {
                        dioxus_utils::console_log(&format!("Status error: {}", err));
                        ctx.write().status = None;
                        data.set(None);
                    }
                }
                let _ = tick.read();
                dioxus_utils::js::sleep(Duration::from_secs(1)).await;
            }
        });
    };

    let snapshot = data.read().clone();
    let thresholds = *use_context::<Signal<HealthThresholds>>().read();

    let content = match snapshot {
        Some(status) => {
            let read_per_second = status.status_bar.read_per_second;
            let write_payloads_per_second = status.status_bar.write_payloads_per_second;
            let write_bytes_per_second = status.status_bar.write_bytes_per_second;
            match status.initialized {
                Some(init) => render_overview(
                    init,
                    thresholds,
                    read_per_second,
                    write_payloads_per_second,
                    write_bytes_per_second,
                ),
                None => render_loading_msg("Server is initializing…"),
            }
        }
        None => render_loading_msg("Connecting to server…"),
    };

    rsx! {
        section { class: "page page--padded", onmounted: on_mount,
            div { class: "overview", {content} }
        }
    }
}

fn render_loading_msg(msg: &str) -> Element {
    rsx! {
        div { class: "empty-state",
            div { class: "empty-state__title", "{msg}" }
        }
    }
}

fn render_overview(
    init: InitializedApiModel,
    thresholds: HealthThresholds,
    read_per_second: u64,
    write_payloads_per_second: u64,
    write_bytes_per_second: u64,
) -> Element {
    let (nodes, readers_only): (Vec<ReaderApiModel>, Vec<ReaderApiModel>) =
        init.readers.iter().cloned().partition(|r| r.is_node);

    let (tone, headline, sub) = compute_health(&readers_only, thresholds);
    let uptime = "—".to_string();

    rsx! {
        HealthBanner { tone, headline, sub, uptime }
        StatsRow {
            tables: init.tables.clone(),
            writers: init.writers.clone(),
            readers: readers_only.clone(),
            nodes: nodes.clone(),
            read_per_second,
            write_payloads_per_second,
            write_bytes_per_second,
        }
        div { class: "two-col",
            div { class: "card",
                div { class: "card__header",
                    span { class: "card__title", "Reader health" }
                    span { class: "card__subtitle", "live · {readers_only.len()} clients" }
                }
                div { class: "card__body",
                    ReaderHealthGrid { readers: readers_only.clone() }
                }
            }
            div { class: "card",
                div { class: "card__header",
                    span { class: "card__title", "Table coverage" }
                    span { class: "card__subtitle", "writers · reads" }
                }
                div { class: "card__body",
                    TableCoverage {
                        tables: init.tables.clone(),
                        writers: init.writers.clone(),
                        readers: readers_only.clone(),
                    }
                }
            }
        }
        NodesTable { nodes }
        WritersTable { writers: init.writers.clone() }
        ReadersTable { readers: readers_only }
    }
}

fn compute_health(
    readers: &[ReaderApiModel],
    thresholds: HealthThresholds,
) -> (HealthTone, String, String) {
    let mut bad = 0usize;
    let mut warn = 0usize;
    for r in readers {
        match classify_reader(&r.last_incoming_time, thresholds.warn_ms, thresholds.bad_ms) {
            StateTone::Bad => bad += 1,
            StateTone::Warn => warn += 1,
            _ => {}
        }
    }

    if bad > 0 {
        (
            HealthTone::Bad,
            format!(
                "{} connection{} are stalled",
                bad,
                if bad == 1 { "" } else { "s" }
            ),
            "One or more reader streams have not received data for over 10 seconds.".to_string(),
        )
    } else if warn > 0 {
        (
            HealthTone::Warn,
            format!(
                "{} connection{} are slow",
                warn,
                if warn == 1 { "" } else { "s" }
            ),
            "Some reader streams are lagging behind the live window.".to_string(),
        )
    } else {
        (
            HealthTone::Ok,
            "All systems nominal".to_string(),
            "All writers and readers are sending data within expected windows.".to_string(),
        )
    }
}
