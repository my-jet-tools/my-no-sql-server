use dioxus::prelude::*;

use crate::utils::format_bytes_per_sec;

#[component]
pub fn Sparkline(values: Vec<u64>, #[props(default)] bytes_label: bool) -> Element {
    let width: f64 = 200.0;
    let height: f64 = 36.0;
    let count = values.len().max(1) as f64;
    let bar_w = (width / count).max(0.5);

    let max = values.iter().copied().max().unwrap_or(0) as f64;
    let coef = if max == 0.0 {
        0.0
    } else {
        (height - 2.0) / max
    };

    let bars = values.iter().enumerate().map(|(i, &v)| {
        let h = (v as f64 * coef).max(0.0);
        let x = i as f64 * bar_w;
        let y = height - h;
        let w = (bar_w - 0.5).max(0.5);
        rsx! {
            rect {
                class: "sparkline__bar",
                x: "{x:.2}",
                y: "{y:.2}",
                width: "{w:.2}",
                height: "{h:.2}",
                rx: "0.5",
            }
        }
    });

    let label_el = if bytes_label && max > 0.0 {
        let txt = format_bytes_per_sec(max);
        rsx! {
            text {
                class: "sparkline__label",
                x: "{width - 2.0}",
                y: "9",
                text_anchor: "end",
                "{txt}"
            }
        }
    } else {
        rsx! {}
    };

    rsx! {
        svg { class: "sparkline", view_box: "0 0 {width} {height}", preserve_aspect_ratio: "none",
            {bars}
            {label_el}
        }
    }
}
