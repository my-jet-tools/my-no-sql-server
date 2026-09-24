use dioxus::prelude::*;

use crate::AppRoute;
use crate::components::atoms::{Icon, IconKind};

#[component]
pub fn Sidebar(
    active: SidebarSection,
    tables_count: usize,
    clients_count: usize,
    /// Clients of the namespace the UI is currently pointed at. This is what the
    /// Connections nav badge shows, because that page lists exactly these.
    clients_in_current_ns: usize,
    /// Readers + writers per namespace, biggest first. Empty while the status
    /// has not arrived yet.
    clients_by_namespace: Vec<(String, usize)>,
    online: bool,
) -> Element {
    let dot_class = if online {
        "sidebar__live-dot"
    } else {
        "sidebar__live-dot offline"
    };
    let live_text = if online {
        format!("Live · {} clients", clients_count)
    } else {
        "Offline".to_string()
    };

    // Only worth the line when there is something to disambiguate: a server
    // running a single namespace says nothing new by naming it.
    let ns_breakdown = if online && clients_by_namespace.len() > 1 {
        let items = clients_by_namespace.into_iter().map(|(namespace, amount)| {
            rsx! {
                span { class: "sidebar__live-ns", key: "{namespace}",
                    span { class: "sidebar__live-ns-name", "{namespace}" }
                    span { class: "sidebar__live-ns-count", "{amount}" }
                }
            }
        });

        rsx! {
            div { class: "sidebar__live-by-ns", {items} }
        }
    } else {
        rsx! {}
    };

    rsx! {
        aside { class: "sidebar",
            div { class: "sidebar__brand",
                div { class: "sidebar__logo",
                    img {
                        class: "sidebar__logo-img",
                        src: asset!("/public/favicon.svg"),
                        alt: "MyNoSql",
                    }
                }
                div {
                    div { class: "sidebar__brand-name", "MyNoSql" }
                    div { class: "sidebar__brand-sub", "v0.7.3 · prod" }
                }
            }
            nav { class: "sidebar__nav",
                Link {
                    to: AppRoute::Home {},
                    class: nav_class(active == SidebarSection::Overview),
                    Icon { kind: IconKind::Activity, class: "sidebar__nav-icon".to_string() }
                    span { class: "sidebar__nav-label", "Overview" }
                }
                Link {
                    to: AppRoute::Data {},
                    class: nav_class(active == SidebarSection::Tables),
                    Icon { kind: IconKind::Database, class: "sidebar__nav-icon".to_string() }
                    span { class: "sidebar__nav-label", "Tables" }
                    span { class: "sidebar__nav-count", "{tables_count}" }
                }
                Link {
                    to: AppRoute::Connections {},
                    class: nav_class(active == SidebarSection::Connections),
                    Icon { kind: IconKind::Plug, class: "sidebar__nav-icon".to_string() }
                    span { class: "sidebar__nav-label", "Connections" }
                    span { class: "sidebar__nav-count", "{clients_in_current_ns}" }
                }
                Link {
                    to: AppRoute::Snapshots {},
                    class: nav_class(active == SidebarSection::Snapshots),
                    Icon { kind: IconKind::HardDrive, class: "sidebar__nav-icon".to_string() }
                    span { class: "sidebar__nav-label", "Snapshots" }
                }
                Link {
                    to: AppRoute::Settings {},
                    class: nav_class(active == SidebarSection::Settings),
                    Icon { kind: IconKind::Settings, class: "sidebar__nav-icon".to_string() }
                    span { class: "sidebar__nav-label", "Settings" }
                }
            }
            div { class: "sidebar__foot",
                div { class: "sidebar__live",
                    div { class: "sidebar__live-line",
                        span { class: dot_class }
                        span { "{live_text}" }
                    }
                    // Which namespaces those clients are in. Without it the
                    // total is ambiguous the moment a second namespace exists.
                    {ns_breakdown}
                }
            }
        }
    }
}

fn nav_class(active: bool) -> &'static str {
    if active {
        "sidebar__nav-item active"
    } else {
        "sidebar__nav-item"
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SidebarSection {
    Overview,
    Tables,
    Connections,
    Snapshots,
    Settings,
}
