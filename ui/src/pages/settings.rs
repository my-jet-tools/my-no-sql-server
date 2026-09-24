use std::time::Duration;

use dioxus::prelude::*;

use crate::api;
use crate::settings::{DEFAULT_BAD_MS, DEFAULT_WARN_MS, HealthThresholds};

/// Remaining time of a write-enable window as `Nm SSs`.
fn format_remaining(secs: u64) -> String {
    format!("{}m {:02}s", secs / 60, secs % 60)
}

#[derive(Default)]
struct SettingsState {
    warn_ms: String,
    bad_ms: String,
    saving: bool,
    message: Option<String>,
    error: Option<String>,
}

#[derive(Default)]
struct McpWritesState {
    /// True while the server-side enable window is open.
    enabled: bool,
    /// Seconds left in the window (None when disabled).
    remaining_secs: Option<u64>,
    /// Set once, so we start the status poller only on first render.
    loaded: bool,
    saving: bool,
    message: Option<String>,
    error: Option<String>,
}

/// Mirrors `McpWritesState` for the UI's own destructive-write window.
/// Kept separate from the MCP one on purpose — enabling writes for an agent
/// must not silently unlock the delete buttons in the UI.
#[derive(Default)]
struct UiWritesState {
    enabled: bool,
    remaining_secs: Option<u64>,
    saving: bool,
    message: Option<String>,
    error: Option<String>,
}

/// Enables/disables UI writes on the server, then re-reads the authoritative
/// state so the countdown reflects the real window.
fn toggle_ui_writes(mut uiw: Signal<UiWritesState>, enabled: bool) {
    {
        let mut w = uiw.write();
        w.saving = true;
        w.error = None;
        w.message = None;
    }
    spawn(async move {
        match api::set_ui_writes(enabled).await {
            Ok(()) => {
                let server = api::get_ui_settings().await.ok();
                let mut w = uiw.write();
                w.saving = false;
                if let Some(s) = server {
                    w.enabled = s.ui_writes_enabled;
                    w.remaining_secs = s.ui_writes_remaining_secs;
                } else {
                    w.enabled = enabled;
                }
                // The server ADDS the window to whatever was left, so a fixed
                // "for 10 minutes" would be wrong on every extension. Report
                // what the server actually says is left.
                w.message = Some(if enabled {
                    match w.remaining_secs {
                        Some(secs) => {
                            format!("Write access enabled. {} left.", format_remaining(secs))
                        }
                        None => "Write access enabled.".to_string(),
                    }
                } else {
                    "Write access disabled.".to_string()
                });
            }
            Err(err) => {
                let mut w = uiw.write();
                w.saving = false;
                w.error = Some(format!("Failed: {}", err));
            }
        }
    });
}

/// Enables/disables MCP writes on the server, then re-reads the
/// authoritative state so the countdown reflects the real window.
fn toggle_mcp_writes(mut mcp: Signal<McpWritesState>, enabled: bool) {
    {
        let mut w = mcp.write();
        w.saving = true;
        w.error = None;
        w.message = None;
    }
    spawn(async move {
        match api::set_mcp_writes(enabled).await {
            Ok(()) => {
                let server = api::get_ui_settings().await.ok();
                let mut w = mcp.write();
                w.saving = false;
                if let Some(s) = server {
                    w.enabled = s.mcp_writes_enabled;
                    w.remaining_secs = s.mcp_writes_remaining_secs;
                } else {
                    w.enabled = enabled;
                }
                w.message = Some(if enabled {
                    match w.remaining_secs {
                        Some(secs) => {
                            format!("MCP writes enabled. {} left.", format_remaining(secs))
                        }
                        None => "MCP writes enabled.".to_string(),
                    }
                } else {
                    "MCP writes disabled.".to_string()
                });
            }
            Err(err) => {
                let mut w = mcp.write();
                w.saving = false;
                w.error = Some(format!("Failed: {}", err));
            }
        }
    });
}

#[component]
pub fn Settings() -> Element {
    let mut thresholds = use_context::<Signal<HealthThresholds>>();
    let mut cs = use_signal(SettingsState::default);
    let mut mcp = use_signal(McpWritesState::default);
    let mut uiw = use_signal(UiWritesState::default);

    // Poll both enable states so the countdowns stay in sync and each card
    // flips back to "Disabled" when its 10-minute window lapses. One poller
    // drives both — `/api/Settings` returns them together.
    {
        let loaded = mcp.read().loaded;
        if !loaded {
            mcp.write().loaded = true;
            spawn(async move {
                loop {
                    if let Ok(s) = api::get_ui_settings().await {
                        {
                            let mut w = mcp.write();
                            w.enabled = s.mcp_writes_enabled;
                            w.remaining_secs = s.mcp_writes_remaining_secs;
                        }
                        let mut w = uiw.write();
                        w.enabled = s.ui_writes_enabled;
                        w.remaining_secs = s.ui_writes_remaining_secs;
                    }
                    dioxus_utils::js::sleep(Duration::from_secs(1)).await;
                }
            });
        }
    }

    // Sync local form fields whenever the context value changes (e.g. on initial load).
    let t = *thresholds.read();
    {
        let mut w = cs.write();
        if w.warn_ms.is_empty() {
            w.warn_ms = t.warn_ms.to_string();
        }
        if w.bad_ms.is_empty() {
            w.bad_ms = t.bad_ms.to_string();
        }
    }

    let cs_ra = cs.read();
    let warn_str = cs_ra.warn_ms.clone();
    let bad_str = cs_ra.bad_ms.clone();
    let saving = cs_ra.saving;
    let message = cs_ra.message.clone();
    let error = cs_ra.error.clone();
    drop(cs_ra);

    let save = move |_| {
        let warn_parsed = cs.read().warn_ms.parse::<u32>();
        let bad_parsed = cs.read().bad_ms.parse::<u32>();
        let (Ok(warn_ms), Ok(bad_ms)) = (warn_parsed, bad_parsed) else {
            let mut w = cs.write();
            w.error = Some("Both fields must be positive whole numbers (ms).".to_string());
            w.message = None;
            return;
        };
        if warn_ms >= bad_ms {
            let mut w = cs.write();
            w.error = Some("Green→Yellow threshold must be smaller than Yellow→Red.".to_string());
            w.message = None;
            return;
        }
        {
            let mut w = cs.write();
            w.saving = true;
            w.error = None;
            w.message = None;
        }
        let new_t = HealthThresholds { warn_ms, bad_ms };
        spawn(async move {
            match api::set_health_thresholds(new_t).await {
                Ok(()) => {
                    thresholds.set(new_t);
                    let mut w = cs.write();
                    w.saving = false;
                    w.message = Some("Saved.".to_string());
                }
                Err(err) => {
                    let mut w = cs.write();
                    w.saving = false;
                    w.error = Some(format!("Save failed: {}", err));
                }
            }
        });
    };

    let reset = move |_| {
        let mut w = cs.write();
        w.warn_ms = DEFAULT_WARN_MS.to_string();
        w.bad_ms = DEFAULT_BAD_MS.to_string();
        w.error = None;
        w.message = None;
    };

    let footer = if let Some(m) = message.clone() {
        rsx! {
            div { style: "color: var(--ok); font-size: 12px;", "{m}" }
        }
    } else if let Some(e) = error.clone() {
        rsx! {
            div { style: "color: var(--danger); font-size: 12px;", "{e}" }
        }
    } else {
        rsx! {}
    };

    // ----- MCP writes card state & handlers -----
    let mcp_ra = mcp.read();
    let mcp_enabled = mcp_ra.enabled;
    let mcp_remaining_secs = mcp_ra.remaining_secs;
    let mcp_saving = mcp_ra.saving;
    let mcp_message = mcp_ra.message.clone();
    let mcp_error = mcp_ra.error.clone();
    drop(mcp_ra);

    let mcp_enable = move |_| toggle_mcp_writes(mcp, true);
    let mcp_disable = move |_| toggle_mcp_writes(mcp, false);

    let mcp_footer = if let Some(m) = mcp_message.clone() {
        rsx! { div { style: "color: var(--ok); font-size: 12px;", "{m}" } }
    } else if let Some(e) = mcp_error.clone() {
        rsx! { div { style: "color: var(--danger); font-size: 12px;", "{e}" } }
    } else {
        rsx! {}
    };

    let mcp_status_label = if mcp_enabled {
        match mcp_remaining_secs {
            Some(secs) => {
                let mins = secs / 60;
                let rem = secs % 60;
                format!("enabled — ~{}m {:02}s left", mins, rem)
            }
            None => "enabled".to_string(),
        }
    } else {
        "disabled".to_string()
    };
    let mcp_status_color = if mcp_enabled {
        "var(--ok)"
    } else {
        "var(--text-muted)"
    };
    let mcp_remaining_label = match mcp_remaining_secs {
        Some(secs) => format_remaining(secs),
        None => "—".to_string(),
    };

    // ----- Write access card state & handlers -----
    let uiw_ra = uiw.read();
    let uiw_enabled = uiw_ra.enabled;
    let uiw_remaining_secs = uiw_ra.remaining_secs;
    let uiw_saving = uiw_ra.saving;
    let uiw_message = uiw_ra.message.clone();
    let uiw_error = uiw_ra.error.clone();
    drop(uiw_ra);

    let uiw_enable = move |_| toggle_ui_writes(uiw, true);
    let uiw_disable = move |_| toggle_ui_writes(uiw, false);

    let uiw_footer = if let Some(m) = uiw_message.clone() {
        rsx! { div { style: "color: var(--ok); font-size: 12px;", "{m}" } }
    } else if let Some(e) = uiw_error.clone() {
        rsx! { div { style: "color: var(--danger); font-size: 12px;", "{e}" } }
    } else {
        rsx! {}
    };

    let uiw_status_label = if uiw_enabled {
        match uiw_remaining_secs {
            Some(secs) => format!("enabled — ~{} left", format_remaining(secs)),
            None => "enabled".to_string(),
        }
    } else {
        "disabled".to_string()
    };
    let uiw_status_color = if uiw_enabled {
        "var(--ok)"
    } else {
        "var(--text-muted)"
    };
    let uiw_remaining_label = match uiw_remaining_secs {
        Some(secs) => format_remaining(secs),
        None => "—".to_string(),
    };

    rsx! {
        section { class: "page page--padded",
            div { style: "max-width: 640px; display: flex; flex-direction: column; gap: 14px;",
                div { class: "card",
                    div { class: "card__header",
                        span { class: "card__title", "Reader health thresholds" }
                        span { class: "card__subtitle", "milliseconds since last incoming" }
                    }
                    div { class: "card__body", style: "display: flex; flex-direction: column; gap: 14px;",
                        p { style: "margin: 0; color: var(--text-muted); font-size: 12.5px;",
                            "Below "
                            b { style: "color: var(--ok); font-family: var(--font-mono);", "Green" }
                            " — healthy. Between Green and Yellow — slow. Above "
                            b { style: "color: var(--danger); font-family: var(--font-mono);", "Yellow" }
                            " — stalled. Values are stored on the server in "
                            code { style: "font-family: var(--font-mono);", "ui-settings.json" }
                            " next to the data files."
                        }

                        div { class: "settings-row",
                            label { class: "settings-row__label",
                                span { class: "state state--ok", span { class: "state__dot" } }
                                "Green → Yellow"
                            }
                            div { class: "settings-row__field",
                                input {
                                    class: "filter-input",
                                    r#type: "number",
                                    min: "0",
                                    value: "{warn_str}",
                                    oninput: move |evt| {
                                        let mut w = cs.write();
                                        w.warn_ms = evt.value();
                                        w.message = None;
                                    },
                                }
                                span { class: "settings-row__unit", "ms" }
                            }
                        }

                        div { class: "settings-row",
                            label { class: "settings-row__label",
                                span { class: "state state--bad", span { class: "state__dot" } }
                                "Yellow → Red"
                            }
                            div { class: "settings-row__field",
                                input {
                                    class: "filter-input",
                                    r#type: "number",
                                    min: "0",
                                    value: "{bad_str}",
                                    oninput: move |evt| {
                                        let mut w = cs.write();
                                        w.bad_ms = evt.value();
                                        w.message = None;
                                    },
                                }
                                span { class: "settings-row__unit", "ms" }
                            }
                        }

                        {footer}
                    }
                    div { class: "card__footer", style: "display: flex; justify-content: flex-end; gap: 6px; padding: 10px 14px;",
                        button { class: "btn btn--ghost btn--sm", onclick: reset, "Reset to defaults" }
                        button {
                            class: "btn btn--primary btn--sm",
                            disabled: saving,
                            onclick: save,
                            if saving { "Saving…" } else { "Save" }
                        }
                    }
                }

                // ----- Write access card (UI writes) -----
                div { class: "card",
                    div { class: "card__header",
                        span { class: "card__title", "Write access" }
                        span {
                            class: "card__subtitle",
                            style: "color: {uiw_status_color};",
                            "{uiw_status_label}"
                        }
                    }
                    div { class: "card__body", style: "display: flex; flex-direction: column; gap: 14px;",
                        p { style: "margin: 0; color: var(--text-muted); font-size: 12.5px;",
                            "Controls destructive operations in this UI ("
                            b { "delete row" }
                            ", "
                            b { "bulk delete" }
                            ", "
                            b { "paste & delete" }
                            ", "
                            b { "restore from backup" }
                            "). They are disabled by default. Click "
                            b { "Enable" }
                            " to allow them for "
                            b { "10 minutes" }
                            "; they auto-disable after that, or click "
                            b { "Disable" }
                            " to turn them off now. A server restart leaves them disabled. "
                            "Browsing data and making backups are always available. "
                            "This window is independent of MCP writes."
                        }

                        if uiw_enabled {
                            div {
                                class: "settings-row",
                                style: "align-items: center;",
                                label { class: "settings-row__label", "Time remaining" }
                                div { class: "settings-row__field",
                                    span {
                                        style: "color: var(--ok); font-family: var(--font-mono); font-weight: 600; font-size: 15px;",
                                        "{uiw_remaining_label}"
                                    }
                                }
                            }
                        }

                        {uiw_footer}
                    }
                    div { class: "card__footer", style: "display: flex; justify-content: flex-end; gap: 6px; padding: 10px 14px;",
                        if uiw_enabled {
                            button {
                                class: "btn btn--ghost btn--sm",
                                disabled: uiw_saving,
                                onclick: uiw_disable,
                                "Disable"
                            }
                            button {
                                class: "btn btn--primary btn--sm",
                                disabled: uiw_saving,
                                onclick: uiw_enable,
                                if uiw_saving { "Working…" } else { "Extend +10 min" }
                            }
                        } else {
                            button {
                                class: "btn btn--primary btn--sm",
                                disabled: uiw_saving,
                                onclick: uiw_enable,
                                if uiw_saving { "Working…" } else { "Enable for 10 min" }
                            }
                        }
                    }
                }

                // ----- MCP writes card -----
                div { class: "card",
                    div { class: "card__header",
                        span { class: "card__title", "MCP writes" }
                        span {
                            class: "card__subtitle",
                            style: "color: {mcp_status_color};",
                            "{mcp_status_label}"
                        }
                    }
                    div { class: "card__body", style: "display: flex; flex-direction: column; gap: 14px;",
                        p { style: "margin: 0; color: var(--text-muted); font-size: 12.5px;",
                            "Controls the MCP write tools ("
                            code { style: "font-family: var(--font-mono);", "delete_row" }
                            ", "
                            code { style: "font-family: var(--font-mono);", "insert_or_replace_row" }
                            ", "
                            code { style: "font-family: var(--font-mono);", "clean_table" }
                            ", …). They are disabled by default. Click "
                            b { "Enable" }
                            " to allow them for "
                            b { "10 minutes" }
                            "; they auto-disable after that, or click "
                            b { "Disable" }
                            " to turn them off now. A server restart leaves them disabled. "
                            "Read-only MCP tools are always available."
                        }

                        if mcp_enabled {
                            div {
                                class: "settings-row",
                                style: "align-items: center;",
                                label { class: "settings-row__label", "Time remaining" }
                                div { class: "settings-row__field",
                                    span {
                                        style: "color: var(--ok); font-family: var(--font-mono); font-weight: 600; font-size: 15px;",
                                        "{mcp_remaining_label}"
                                    }
                                }
                            }
                        }

                        {mcp_footer}
                    }
                    div { class: "card__footer", style: "display: flex; justify-content: flex-end; gap: 6px; padding: 10px 14px;",
                        if mcp_enabled {
                            button {
                                class: "btn btn--ghost btn--sm",
                                disabled: mcp_saving,
                                onclick: mcp_disable,
                                "Disable"
                            }
                            button {
                                class: "btn btn--primary btn--sm",
                                disabled: mcp_saving,
                                onclick: mcp_enable,
                                if mcp_saving { "Working…" } else { "Extend +10 min" }
                            }
                        } else {
                            button {
                                class: "btn btn--primary btn--sm",
                                disabled: mcp_saving,
                                onclick: mcp_enable,
                                if mcp_saving { "Working…" } else { "Enable for 10 min" }
                            }
                        }
                    }
                }
            }
        }
    }
}
