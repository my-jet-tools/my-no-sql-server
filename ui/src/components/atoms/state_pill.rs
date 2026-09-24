use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum StateTone {
    Ok,
    Warn,
    Bad,
    Neutral,
}

impl StateTone {
    fn class(&self) -> &'static str {
        match self {
            StateTone::Ok => "state--ok",
            StateTone::Warn => "state--warn",
            StateTone::Bad => "state--bad",
            StateTone::Neutral => "state--neutral",
        }
    }
}

#[component]
pub fn StatePill(label: String, tone: StateTone) -> Element {
    rsx! {
        span { class: "state {tone.class()}",
            span { class: "state__dot" }
            span { "{label}" }
        }
    }
}

#[component]
pub fn StatusDot(tone: StateTone) -> Element {
    rsx! {
        span { class: "state {tone.class()}",
            span { class: "state__dot" }
        }
    }
}

/// Classify a reader's freshness based on its `lastIncomingTime` string.
///
/// The server emits durations like `"0.123s"`, `"2.5s"`, `"00:00:14"`, or
/// `"00:00:00.450"`. We only need rough buckets: ok / warn / bad.
///
/// Thresholds are given in milliseconds — `warn_ms` is the lower bound for
/// the yellow zone, `bad_ms` for the red zone. Anything below `warn_ms` is OK.
pub fn classify_reader(last_incoming: &str, warn_ms: u32, bad_ms: u32) -> StateTone {
    let ms = parse_secs(last_incoming) * 1000.0;
    if ms >= bad_ms as f64 {
        StateTone::Bad
    } else if ms >= warn_ms as f64 {
        StateTone::Warn
    } else {
        StateTone::Ok
    }
}

fn parse_secs(s: &str) -> f64 {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return 0.0;
    }

    // hh:mm:ss[.fff]
    if trimmed.contains(':') {
        let main = trimmed;
        let parts: Vec<&str> = main.split(':').collect();
        let mut acc = 0.0_f64;
        let mut mult = match parts.len() {
            3 => 3600.0,
            2 => 60.0,
            _ => 1.0,
        };
        for p in &parts {
            acc += p.parse::<f64>().unwrap_or(0.0) * mult;
            mult /= 60.0;
        }
        return acc;
    }

    // "1.234s" or "750ms" or bare number
    let lower = trimmed.to_ascii_lowercase();
    if let Some(num) = lower.strip_suffix("ms") {
        return num.trim().parse::<f64>().unwrap_or(0.0) / 1000.0;
    }
    if let Some(num) = lower.strip_suffix("us") {
        return num.trim().parse::<f64>().unwrap_or(0.0) / 1_000_000.0;
    }
    if let Some(num) = lower.strip_suffix('s') {
        return num.trim().parse::<f64>().unwrap_or(0.0);
    }
    lower.parse::<f64>().unwrap_or(0.0)
}
