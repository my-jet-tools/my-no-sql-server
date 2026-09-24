use dioxus::prelude::*;

use super::format_compact_count;

pub const PAGE_SIZE_OPTIONS: &[usize] = &[50, 100, 200, 500];

#[component]
pub fn TablePagination(
    total: usize,
    page_size: usize,
    current_page: usize,
    on_page_change: EventHandler<usize>,
    on_page_size_change: EventHandler<usize>,
) -> Element {
    if total == 0 {
        return rsx! {};
    }

    let total_pages = total.div_ceil(page_size).max(1);
    let page = current_page.min(total_pages - 1);
    let start = page * page_size + 1;
    let end = ((page + 1) * page_size).min(total);
    let is_first = page == 0;
    let is_last = page + 1 >= total_pages;

    let info = format!(
        "Showing {}–{} of {}",
        start,
        end,
        format_compact_count(total as u64)
    );
    let page_label = format!("Page {} of {}", page + 1, total_pages);

    let prev_page = if is_first { 0 } else { page - 1 };
    let next_page = if is_last { page } else { page + 1 };
    let last_page = total_pages - 1;

    let size_options = PAGE_SIZE_OPTIONS.iter().map(|&sz| {
        let selected = sz == page_size;
        rsx! {
            option {
                value: "{sz}",
                selected: selected,
                "{sz} / page"
            }
        }
    });

    rsx! {
        div { class: "table-pagination",
            div { class: "table-pagination__info", "{info}" }
            div { class: "table-pagination__spacer" }
            select {
                class: "table-pagination__page-size",
                value: "{page_size}",
                onchange: move |evt| {
                    if let Ok(sz) = evt.value().parse::<usize>() {
                        on_page_size_change.call(sz);
                    }
                },
                {size_options}
            }
            div { class: "table-pagination__controls",
                button {
                    class: "btn btn--sm table-pagination__btn",
                    disabled: is_first,
                    onclick: move |_| on_page_change.call(0),
                    "«"
                }
                button {
                    class: "btn btn--sm table-pagination__btn",
                    disabled: is_first,
                    onclick: move |_| on_page_change.call(prev_page),
                    "‹"
                }
                span { class: "table-pagination__label", "{page_label}" }
                button {
                    class: "btn btn--sm table-pagination__btn",
                    disabled: is_last,
                    onclick: move |_| on_page_change.call(next_page),
                    "›"
                }
                button {
                    class: "btn btn--sm table-pagination__btn",
                    disabled: is_last,
                    onclick: move |_| on_page_change.call(last_page),
                    "»"
                }
            }
        }
    }
}
