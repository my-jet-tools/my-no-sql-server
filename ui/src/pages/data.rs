use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use dioxus::prelude::*;
use serde_json::Value;

use crate::AppContext;
use crate::AppRoute;
use crate::api::{
    bulk_delete_many, bulk_delete_rows, delete_row, get_partition_details, get_rows, get_status,
    get_tables_list,
};
use crate::components::atoms::{Badge, BadgeTone, Icon, IconKind};
use crate::components::data::{
    PARTITION_KEY, PartitionsPane, ROW_KEY, RowDrawer, RowsTable, TIME_STAMP, TableHeader,
    TableListMetrics, TablePagination, TableToolbar, TablesPane,
};
use crate::models::{
    PartitionMetricApiModel, StatusApiModel, TableApiModel, TableListItemApiModel,
};

#[derive(Clone)]
enum DialogState {
    DeleteOne {
        partition_key: String,
        row_key: String,
    },
    BulkDelete {
        partition_key: String,
        row_keys: Vec<String>,
    },
    PasteDelete {
        raw: String,
        parsed: Option<BTreeMap<String, Vec<String>>>,
        total_rows: usize,
        partitions_touched: usize,
        error: Option<String>,
    },
    /// Turn in-memory compression on or off for the selected table.
    /// `compressed` is the state the server reported when the dialog opened, so
    /// the dialog always offers the opposite of it.
    Compression {
        table: String,
        compressed: bool,
        /// Also re-encode the rows already in memory, not just future writes.
        force: bool,
        /// Set while the request is in flight — re-encoding a big table is not
        /// instant, and a second click would fire a second walk over it.
        busy: bool,
    },
}

struct DataState {
    tables: Vec<TableListItemApiModel>,
    tables_loaded: bool,
    /// Table whose partition list is currently loaded or loading.
    loaded_for_table: Option<String>,
    partitions: Option<Vec<String>>,
    /// Per-partition metrics (records + bytes) for `loaded_for_table`, refreshed
    /// in the background every few seconds.
    partition_metrics: HashMap<String, PartitionMetricApiModel>,
    /// (table, partition) whose rows are currently loaded or loading.
    loaded_rows_for: Option<(String, String)>,
    /// True once the rows for `loaded_rows_for` have actually arrived.
    rows_ready: bool,
    headers: Vec<String>,
    rows: Vec<Value>,
    dialog: Option<DialogState>,
    /// Why the last write attempt from the open dialog failed — most often
    /// "write access is disabled". Rendered inside the dialog, since a
    /// rejected delete otherwise looks like the button did nothing.
    write_error: Option<String>,
    checked_keys: HashSet<String>,
    page_size: usize,
    current_page: usize,
}

impl Default for DataState {
    fn default() -> Self {
        Self {
            tables: Vec::new(),
            tables_loaded: false,
            loaded_for_table: None,
            partitions: None,
            partition_metrics: HashMap::new(),
            loaded_rows_for: None,
            rows_ready: false,
            headers: Vec::new(),
            rows: Vec::new(),
            dialog: None,
            write_error: None,
            checked_keys: HashSet::new(),
            page_size: DEFAULT_PAGE_SIZE,
            current_page: 0,
        }
    }
}

const DEFAULT_PAGE_SIZE: usize = 100;

impl DataState {
    fn set_tables(&mut self, tables: Vec<TableListItemApiModel>) {
        self.tables = tables;
        self.tables_loaded = true;
    }

    fn mark_tables_loaded(&mut self) {
        self.tables_loaded = true;
    }

    /// Start loading the partition list for `table`; invalidates any rows.
    fn begin_table_load(&mut self, table: &str) {
        self.loaded_for_table = Some(table.to_string());
        self.partitions = None;
        self.partition_metrics.clear();
        self.loaded_rows_for = None;
        self.rows_ready = false;
        self.headers = Vec::new();
        self.rows = Vec::new();
        self.checked_keys.clear();
        self.current_page = 0;
    }

    /// Apply fetched per-partition details (keys + metrics), unless the table was
    /// switched meanwhile. Rebuilds both the ordered key list and the metrics map.
    fn set_partition_details(&mut self, table: &str, details: Vec<PartitionMetricApiModel>) {
        if self.loaded_for_table.as_deref() != Some(table) {
            return;
        }
        let mut keys = Vec::with_capacity(details.len());
        let mut metrics = HashMap::with_capacity(details.len());
        for d in details {
            keys.push(d.partition_key.clone());
            metrics.insert(d.partition_key.clone(), d);
        }
        self.partitions = Some(keys);
        self.partition_metrics = metrics;
    }

    /// Start loading rows for `(table, partition)`; clears the previous rows.
    fn begin_rows_load(&mut self, table: &str, partition: &str) {
        self.loaded_rows_for = Some((table.to_string(), partition.to_string()));
        self.rows_ready = false;
        self.headers = Vec::new();
        self.rows = Vec::new();
        self.checked_keys.clear();
        self.current_page = 0;
    }

    fn set_page(&mut self, page: usize) {
        self.current_page = page;
    }

    fn set_page_size(&mut self, size: usize) {
        self.page_size = size.max(1);
        self.current_page = 0;
    }

    fn reset_pagination(&mut self) {
        self.current_page = 0;
    }

    /// Apply fetched rows, unless the (table, partition) changed meanwhile.
    fn set_rows(&mut self, table: &str, partition: &str, rows: Vec<Value>) {
        let matches = self
            .loaded_rows_for
            .as_ref()
            .map(|(t, p)| t.as_str() == table && p.as_str() == partition)
            .unwrap_or(false);
        if matches {
            let (headers, rows) = build_rows_state(rows);
            self.headers = headers;
            self.rows = rows;
            self.rows_ready = true;
        }
    }

    /// Force the next render to refetch rows for the current partition.
    fn clear_rows_scope(&mut self) {
        self.loaded_rows_for = None;
        self.rows_ready = false;
    }

    /// Opens a confirm dialog. Clears any error left over from a previous
    /// attempt, so a reopened dialog never shows a stale rejection.
    fn open_dialog(&mut self, dialog: DialogState) {
        self.dialog = Some(dialog);
        self.write_error = None;
    }

    fn close_dialog(&mut self) {
        self.dialog = None;
        self.write_error = None;
    }

    fn set_write_error(&mut self, err: String) {
        self.write_error = Some(err);
    }
}

/// Extract `(table, partition, row)` from the current data route.
fn parse_data_route(route: &AppRoute) -> (Option<String>, Option<String>, Option<String>) {
    match route {
        AppRoute::DataTable { table } => (Some(table.clone()), None, None),
        AppRoute::DataPartition { table, partition } => {
            (Some(table.clone()), Some(partition.clone()), None)
        }
        AppRoute::DataRow {
            table,
            partition,
            row,
        } => (
            Some(table.clone()),
            Some(partition.clone()),
            Some(row.clone()),
        ),
        _ => (None, None, None),
    }
}

// Route placeholders — the URL patterns for the data section. `DataLayout`
// renders the whole page and reads the params via `use_route`, so these render
// nothing themselves.
#[component]
pub fn Data() -> Element {
    rsx! {}
}

#[component]
pub fn DataTable(table: String) -> Element {
    let _ = table;
    rsx! {}
}

#[component]
pub fn DataPartition(table: String, partition: String) -> Element {
    let _ = (table, partition);
    rsx! {}
}

#[component]
pub fn DataRow(table: String, partition: String, row: String) -> Element {
    let _ = (table, partition, row);
    rsx! {}
}

#[component]
pub fn DataLayout() -> Element {
    let mut cs = use_signal(DataState::default);
    let row_filter = use_signal(String::new);
    let app_ctx = use_context::<Signal<AppContext>>();
    let nav = navigator();

    let route = use_route::<AppRoute>();
    let (url_table, url_partition, url_row) = parse_data_route(&route);

    // ---- one-time tables list load ----
    use_effect(move || {
        spawn(async move {
            if cs.peek().tables_loaded {
                return;
            }
            match get_tables_list().await {
                Ok(list) => {
                    let mut sorted = list;
                    sorted.sort_by(|a, b| a.name.cmp(&b.name));
                    cs.write().set_tables(sorted);
                }
                Err(err) => {
                    dioxus_utils::console_log(&format!("Tables error: {}", err));
                    cs.write().mark_tables_loaded();
                }
            }
        });
    });

    // ---- background status refresh: keeps the tables-pane partition + record
    // counts live while the data section is mounted. Polls every 3s. ----
    let mut status_started = use_signal(|| false);
    use_effect(move || {
        if *status_started.peek() {
            return;
        }
        status_started.set(true);
        let mut ctx = app_ctx;
        spawn(async move {
            loop {
                match get_status().await {
                    Ok(s) => ctx.write().status = Some(s),
                    Err(err) => {
                        dioxus_utils::console_log(&format!("Status error: {}", err));
                    }
                }
                dioxus_utils::js::sleep(Duration::from_secs(3)).await;
            }
        });
    });

    // ---- reset to page 1 whenever the filter changes ----
    use_effect(move || {
        let _ = row_filter.read();
        if cs.peek().current_page != 0 {
            cs.write().reset_pagination();
        }
    });

    // ---- load + live-refresh the partition list (keys + metrics) for the URL
    // table. A per-table loop fetches once immediately and then re-polls every 3s
    // so record counts and byte sizes stay current. It self-terminates when the
    // selected table changes (a fresh render spawns a new loop for the new table).
    if let Some(table) = url_table.clone() {
        let already = { cs.read().loaded_for_table.as_deref() == Some(table.as_str()) };
        if !already {
            let url_partition_at_nav = url_partition.clone();
            spawn(async move {
                if cs.peek().loaded_for_table.as_deref() == Some(table.as_str()) {
                    return;
                }
                cs.write().begin_table_load(&table);
                let mut first = true;
                loop {
                    if cs.peek().loaded_for_table.as_deref() != Some(table.as_str()) {
                        break;
                    }
                    match get_partition_details(&table).await {
                        Ok(details) => {
                            // Table with a single partition — jump straight into it
                            // on first load. `replace` so Back skips this step.
                            let only = if details.len() == 1 {
                                details.first().map(|d| d.partition_key.clone())
                            } else {
                                None
                            };
                            cs.write().set_partition_details(&table, details);
                            if first && url_partition_at_nav.is_none() {
                                if let Some(only) = only {
                                    nav.replace(AppRoute::DataPartition {
                                        table: table.clone(),
                                        partition: only,
                                    });
                                }
                            }
                        }
                        Err(err) => {
                            dioxus_utils::console_log(&format!("Partitions error: {}", err));
                        }
                    }
                    first = false;
                    dioxus_utils::js::sleep(Duration::from_secs(3)).await;
                }
            });
        }
    }

    // ---- load rows whenever the URL (table, partition) changes ----
    if let (Some(table), Some(partition)) = (url_table.clone(), url_partition.clone()) {
        let pair = (table, partition);
        let already = { cs.read().loaded_rows_for.as_ref() == Some(&pair) };
        if !already {
            spawn(async move {
                if cs.peek().loaded_rows_for.as_ref() == Some(&pair) {
                    return;
                }
                cs.write().begin_rows_load(&pair.0, &pair.1);
                match get_rows(&pair.0, &pair.1).await {
                    Ok(rows) => cs.write().set_rows(&pair.0, &pair.1, rows),
                    Err(err) => {
                        dioxus_utils::console_log(&format!("Rows error: {}", err));
                    }
                }
            });
        }
    }

    // ---- read state for rendering ----
    let cs_ra = cs.read();
    let tables = cs_ra.tables.clone();

    let partitions_list: Vec<String> =
        match (&url_table, &cs_ra.loaded_for_table, &cs_ra.partitions) {
            (Some(t), Some(lt), Some(list)) if t == lt => list.clone(),
            _ => Vec::new(),
        };

    let partition_metrics: HashMap<String, PartitionMetricApiModel> =
        match (&url_table, &cs_ra.loaded_for_table) {
            (Some(t), Some(lt)) if t == lt => cs_ra.partition_metrics.clone(),
            _ => HashMap::new(),
        };

    let rows_scope_matches = match (&url_table, &url_partition, &cs_ra.loaded_rows_for) {
        (Some(t), Some(p), Some((lt, lp))) => t == lt && p == lp,
        _ => false,
    };
    let rows_ready = rows_scope_matches && cs_ra.rows_ready;
    let headers = if rows_scope_matches {
        cs_ra.headers.clone()
    } else {
        Vec::new()
    };
    let all_rows = if rows_scope_matches {
        cs_ra.rows.clone()
    } else {
        Vec::new()
    };
    let checked_keys = cs_ra.checked_keys.clone();
    let dialog_val = cs_ra.dialog.clone();
    let write_error = cs_ra.write_error.clone();
    let page_size = cs_ra.page_size;
    let stored_page = cs_ra.current_page;
    drop(cs_ra);

    let selected_table = url_table.clone().unwrap_or_default();

    // Derive writers/readers/stats for the selected table
    let ctx_ra = app_ctx.read();
    let status: Option<StatusApiModel> = ctx_ra.status.clone();
    drop(ctx_ra);

    let writer_tables: HashSet<String> = build_writer_tables(&status);
    let (writer_apps_for_selected, reader_count_for_selected) =
        derive_table_connectivity(&status, &selected_table);
    let table_stats = derive_table_stats(&status, &selected_table);
    let table_metrics = derive_table_metrics(&status);

    // Filter rows
    let filter_str = row_filter.read().to_lowercase();
    let filtered_rows: Vec<Value> = if filter_str.is_empty() {
        all_rows.clone()
    } else {
        all_rows
            .iter()
            .filter(|row| row.to_string().to_lowercase().contains(&filter_str))
            .cloned()
            .collect()
    };

    // Paginate the filtered set — clamp during render (no signal write here).
    let filtered_total = filtered_rows.len();
    let total_pages = filtered_total.div_ceil(page_size).max(1);
    let current_page = stored_page.min(total_pages - 1);
    let page_start = current_page * page_size;
    let page_end = (page_start + page_size).min(filtered_total);
    let visible_rows: Vec<Value> = filtered_rows[page_start..page_end].to_vec();
    let visible_keys: Vec<String> = visible_rows
        .iter()
        .filter_map(|r| r.get(ROW_KEY).and_then(|v| v.as_str().map(String::from)))
        .collect();

    // The drawer carries only the row key in the URL — resolve the full row.
    let resolved_row: Option<Value> = url_row.as_ref().and_then(|rk| {
        all_rows
            .iter()
            .find(|r| r.get(ROW_KEY).and_then(|v| v.as_str()) == Some(rk.as_str()))
            .cloned()
    });

    // ---- navigation handlers ----
    let select_table = move |name: String| {
        nav.push(AppRoute::DataTable { table: name });
    };

    let select_partition = {
        let table = url_table.clone();
        move |pk: String| {
            if let Some(table) = table.clone() {
                nav.push(AppRoute::DataPartition {
                    table,
                    partition: pk,
                });
            }
        }
    };

    let on_row_click = {
        let table = url_table.clone();
        let partition = url_partition.clone();
        move |row: Value| {
            let (Some(table), Some(partition)) = (table.clone(), partition.clone()) else {
                return;
            };
            let rk = row
                .get(ROW_KEY)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            nav.push(AppRoute::DataRow {
                table,
                partition,
                row: rk,
            });
        }
    };

    let close_drawer = {
        let table = url_table.clone();
        let partition = url_partition.clone();
        move |_| {
            if let (Some(table), Some(partition)) = (table.clone(), partition.clone()) {
                nav.push(AppRoute::DataPartition { table, partition });
            }
        }
    };

    let confirm_delete = {
        let table = url_table.clone();
        let partition = url_partition.clone();
        move |_| {
            let dialog_val = cs.read().dialog.clone();
            let Some(table_name) = table.clone() else {
                return;
            };
            match dialog_val {
                Some(DialogState::DeleteOne {
                    partition_key,
                    row_key,
                }) => {
                    let back_partition = partition.clone();
                    spawn(async move {
                        if let Err(err) = delete_row(&table_name, &partition_key, &row_key).await {
                            cs.write().set_write_error(err.to_string());
                            return;
                        }
                        {
                            let mut w = cs.write();
                            w.close_dialog();
                            w.checked_keys.remove(&row_key);
                            w.clear_rows_scope();
                        }
                        // Drop the row segment so the drawer closes.
                        if let Some(partition) = back_partition {
                            nav.push(AppRoute::DataPartition {
                                table: table_name.clone(),
                                partition,
                            });
                        }
                    });
                }
                Some(DialogState::BulkDelete {
                    partition_key,
                    row_keys,
                }) => {
                    spawn(async move {
                        if let Err(err) =
                            bulk_delete_rows(&table_name, &partition_key, &row_keys).await
                        {
                            cs.write().set_write_error(err.to_string());
                            return;
                        }
                        let mut w = cs.write();
                        w.close_dialog();
                        for rk in &row_keys {
                            w.checked_keys.remove(rk);
                        }
                        w.clear_rows_scope();
                    });
                }
                Some(DialogState::PasteDelete {
                    parsed: Some(grouped),
                    ..
                }) => {
                    spawn(async move {
                        if let Err(err) = bulk_delete_many(&table_name, &grouped).await {
                            cs.write().set_write_error(err.to_string());
                            return;
                        }
                        let mut w = cs.write();
                        w.close_dialog();
                        w.checked_keys.clear();
                        w.clear_rows_scope();
                    });
                }
                Some(DialogState::PasteDelete { parsed: None, .. }) => {}
                // Not a delete — the compression dialog confirms through its own
                // handler, built where it is rendered.
                Some(DialogState::Compression { .. }) => {}
                None => {}
            }
        }
    };

    let toggle_row_check = move |rk: String| {
        let mut w = cs.write();
        if w.checked_keys.contains(&rk) {
            w.checked_keys.remove(&rk);
        } else {
            w.checked_keys.insert(rk);
        }
    };

    let toggle_all_check = {
        let visible_keys = visible_keys.clone();
        move |check_all: bool| {
            let mut w = cs.write();
            if check_all {
                for k in &visible_keys {
                    w.checked_keys.insert(k.clone());
                }
            } else {
                for k in &visible_keys {
                    w.checked_keys.remove(k);
                }
            }
        }
    };

    let center_content = if url_table.is_none() {
        render_empty_state(tables.clone(), select_table)
    } else {
        let on_refresh_table = move |_| {
            cs.write().clear_rows_scope();
        };
        let on_compression_click = {
            let table = selected_table.clone();
            let compressed = table_stats.as_ref().map(|t| t.compressed).unwrap_or(false);
            move |_| {
                cs.write().open_dialog(DialogState::Compression {
                    table: table.clone(),
                    compressed,
                    force: false,
                    busy: false,
                });
            }
        };
        let on_export_click = {
            let table = url_table.clone();
            let pk_opt = url_partition.clone();
            move |_| {
                let (Some(table), Some(pk)) = (table.clone(), pk_opt.clone()) else {
                    return;
                };
                let url = crate::api::download_rows_url(&table, &pk);
                let script = format!(
                    "window.location.href = {};",
                    serde_json::to_string(&url).unwrap_or_else(|_| "\"\"".to_string())
                );
                let _ = dioxus::document::eval(&script);
            }
        };
        let export_enabled = url_partition.is_some();
        let checked_in_partition: Vec<String> = filtered_rows
            .iter()
            .filter_map(|r| {
                r.get(ROW_KEY)
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
            })
            .filter(|k| checked_keys.contains(k))
            .collect();
        let checked_count = checked_in_partition.len();

        let bulk_bar = if checked_count > 0 {
            let pk_opt = url_partition.clone();
            let keys_for_delete = checked_in_partition.clone();
            rsx! {
                div { class: "bulk-bar",
                    span { class: "bulk-bar__count", "{checked_count} selected" }
                    div { class: "bulk-bar__spacer" }
                    button {
                        class: "btn btn--ghost btn--sm",
                        onclick: move |_| { cs.write().checked_keys.clear(); },
                        "Clear"
                    }
                    button {
                        class: "btn btn--danger btn--sm",
                        onclick: move |_| {
                            let Some(pk) = pk_opt.clone() else { return };
                            cs.write().open_dialog(DialogState::BulkDelete {
                                partition_key: pk,
                                row_keys: keys_for_delete.clone(),
                            });
                        },
                        "Delete selected"
                    }
                }
            }
        } else {
            rsx! {}
        };

        rsx! {
            div { class: "rows-col",
                TableHeader {
                    name: selected_table.clone(),
                    stats: table_stats.clone(),
                    on_refresh: on_refresh_table,
                    on_compression: on_compression_click,
                }
                TableToolbar {
                    filter_value: row_filter,
                    writer_tags: writer_apps_for_selected.clone(),
                    reader_count: reader_count_for_selected,
                    on_export: on_export_click,
                    export_enabled,
                    on_paste_delete: move |_| {
                        cs.write().open_dialog(DialogState::PasteDelete {
                            raw: String::new(),
                            parsed: None,
                            total_rows: 0,
                            partitions_touched: 0,
                            error: None,
                        });
                    },
                    paste_enabled: true,
                }
                {bulk_bar}
                RowsTable {
                    headers: headers.clone(),
                    rows: visible_rows,
                    selected_row_key: url_row.clone(),
                    on_row_click,
                    selectable: true,
                    checked_keys: checked_keys.clone(),
                    on_toggle_row: toggle_row_check,
                    on_toggle_all: toggle_all_check,
                }
                TablePagination {
                    total: filtered_total,
                    page_size,
                    current_page,
                    on_page_change: move |p: usize| { cs.write().set_page(p); },
                    on_page_size_change: move |sz: usize| { cs.write().set_page_size(sz); },
                }
            }
        }
    };

    let partitions_content = if url_table.is_none() {
        rsx! { aside { class: "partitions-pane" } }
    } else {
        rsx! {
            PartitionsPane {
                partitions: partitions_list,
                metrics: partition_metrics,
                selected: url_partition.clone(),
                on_select: move |pk| select_partition(pk),
            }
        }
    };

    let drawer_content = match url_row.as_ref() {
        None => rsx! {},
        Some(rk) => {
            if !rows_ready {
                rsx! {
                    DrawerMessage {
                        title: "Loading row…".to_string(),
                        message: "Fetching partition rows…".to_string(),
                        on_close: close_drawer,
                    }
                }
            } else if let Some(row) = resolved_row.clone() {
                let pk_val = row
                    .get(PARTITION_KEY)
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                let rk_val = row
                    .get(ROW_KEY)
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                rsx! {
                    RowDrawer {
                        row,
                        on_close: close_drawer,
                        on_delete: move |_| {
                            cs.write().open_dialog(DialogState::DeleteOne {
                                partition_key: pk_val.clone(),
                                row_key: rk_val.clone(),
                            });
                        },
                    }
                }
            } else {
                rsx! {
                    DrawerMessage {
                        title: "Row not found".to_string(),
                        message: format!("No row with key \"{}\" in this partition.", rk),
                        on_close: close_drawer,
                    }
                }
            }
        }
    };

    // A rejected write (most often "write access is disabled") is shown inside
    // the open dialog — the dialog deliberately stays open so the user can
    // enable write access and retry without re-selecting the rows.
    let write_error_line = match write_error.as_ref() {
        Some(msg) => rsx! { div { class: "dialog__error", "{msg}" } },
        None => rsx! {},
    };

    let dialog_render = match dialog_val {
        Some(DialogState::DeleteOne {
            partition_key,
            row_key,
        }) => rsx! {
            div { class: "dialog-overlay",
                div { class: "dialog",
                    div { class: "dialog__header", "Confirm delete" }
                    div { class: "dialog__body",
                        "Delete row "
                        b { "{row_key}" }
                        " from partition "
                        b { "{partition_key}" }
                        "?"
                        {write_error_line.clone()}
                    }
                    div { class: "dialog__footer",
                        button {
                            class: "btn btn--ghost btn--sm",
                            onclick: move |_| { cs.write().close_dialog(); },
                            "Cancel"
                        }
                        button { class: "btn btn--danger btn--sm", onclick: confirm_delete.clone(),
                            "Delete"
                        }
                    }
                }
            }
        },
        Some(DialogState::BulkDelete {
            partition_key,
            row_keys,
        }) => {
            let total = row_keys.len();
            const PREVIEW_LIMIT: usize = 50;
            let preview: Vec<String> = row_keys.iter().take(PREVIEW_LIMIT).cloned().collect();
            let extra = total.saturating_sub(preview.len());
            let items = preview.into_iter().map(|k| {
                rsx! {
                    div { class: "dialog__list-item", "{k}" }
                }
            });
            let extra_line = if extra > 0 {
                rsx! { div { class: "dialog__list-extra", "+ {extra} more" } }
            } else {
                rsx! {}
            };
            rsx! {
                div { class: "dialog-overlay",
                    div { class: "dialog",
                        div { class: "dialog__header", "Confirm bulk delete" }
                        div { class: "dialog__body",
                            "Delete "
                            b { "{total}" }
                            " row(s) from partition "
                            b { "{partition_key}" }
                            "?"
                            div { class: "dialog__list",
                                {items}
                                {extra_line}
                            }
                            {write_error_line.clone()}
                        }
                        div { class: "dialog__footer",
                            button {
                                class: "btn btn--ghost btn--sm",
                                onclick: move |_| { cs.write().close_dialog(); },
                                "Cancel"
                            }
                            button { class: "btn btn--danger btn--sm", onclick: confirm_delete.clone(),
                                "Delete {total} row(s)"
                            }
                        }
                    }
                }
            }
        }
        Some(DialogState::PasteDelete {
            raw,
            parsed,
            total_rows,
            partitions_touched,
            error,
        }) => {
            let parse_ready = parsed.is_some();
            let total = total_rows;
            let partitions_n = partitions_touched;
            let error_text = error.clone();
            let target_table = selected_table.clone();

            let on_textarea_input = move |evt: dioxus::events::FormEvent| {
                let new_raw = evt.value();
                let mut w = cs.write();
                if let Some(DialogState::PasteDelete {
                    raw: r,
                    parsed: p,
                    total_rows: t,
                    partitions_touched: pt,
                    error: e,
                }) = w.dialog.as_mut()
                {
                    *r = new_raw;
                    *p = None;
                    *t = 0;
                    *pt = 0;
                    *e = None;
                }
            };

            let on_parse_click = move |_| {
                let raw_snapshot = {
                    let cs_ra = cs.read();
                    match cs_ra.dialog.as_ref() {
                        Some(DialogState::PasteDelete { raw, .. }) => raw.clone(),
                        _ => return,
                    }
                };
                match parse_paste_delete_input(&raw_snapshot) {
                    Ok((grouped, total, partitions_touched)) => {
                        let mut w = cs.write();
                        if let Some(DialogState::PasteDelete {
                            parsed,
                            total_rows,
                            partitions_touched: pt_field,
                            error,
                            ..
                        }) = w.dialog.as_mut()
                        {
                            *parsed = Some(grouped);
                            *total_rows = total;
                            *pt_field = partitions_touched;
                            *error = None;
                        }
                    }
                    Err(msg) => {
                        let mut w = cs.write();
                        if let Some(DialogState::PasteDelete {
                            parsed,
                            total_rows,
                            partitions_touched: pt_field,
                            error,
                            ..
                        }) = w.dialog.as_mut()
                        {
                            *parsed = None;
                            *total_rows = 0;
                            *pt_field = 0;
                            *error = Some(msg);
                        }
                    }
                }
            };

            let preview_items: Vec<(String, String)> = parsed
                .as_ref()
                .map(|g| {
                    const PREVIEW_LIMIT: usize = 50;
                    let mut out = Vec::new();
                    'outer: for (pk, rks) in g.iter() {
                        for rk in rks {
                            out.push((pk.clone(), rk.clone()));
                            if out.len() >= PREVIEW_LIMIT {
                                break 'outer;
                            }
                        }
                    }
                    out
                })
                .unwrap_or_default();
            let preview_shown = preview_items.len();
            let preview_extra = total.saturating_sub(preview_shown);
            let preview_items_render = preview_items.into_iter().map(|(pk, rk)| {
                rsx! {
                    div { class: "dialog__list-item", "pk={pk} / rk={rk}" }
                }
            });
            let preview_extra_line = if preview_extra > 0 {
                rsx! { div { class: "dialog__list-extra", "+ {preview_extra} more" } }
            } else {
                rsx! {}
            };

            let placeholder_text: &'static str = "[\n  { \"PartitionKey\": \"a\", \"RowKey\": \"1\" },\n  { \"PartitionKey\": \"b\", \"RowKey\": \"5\" }\n]";

            let error_render = if let Some(msg) = error_text {
                rsx! {
                    div { class: "dialog__error", "{msg}" }
                }
            } else {
                rsx! {}
            };

            let summary_render = if parse_ready {
                rsx! {
                    div { style: "margin: 6px 0;",
                        "Will delete "
                        b { "{total}" }
                        " row(s) across "
                        b { "{partitions_n}" }
                        " partition(s)"
                    }
                }
            } else {
                rsx! {}
            };

            rsx! {
                div { class: "dialog-overlay",
                    div { class: "dialog",
                        div { class: "dialog__header", "Paste & delete" }
                        div { class: "dialog__body",
                            div { style: "margin-bottom: 6px;",
                                "Target table: "
                                b { "{target_table}" }
                            }
                            div { style: "margin-bottom: 6px; font-size: 12px; color: gray;",
                                "Paste a JSON array of objects with "
                                code { "PartitionKey" }
                                " and "
                                code { "RowKey" }
                                " fields."
                            }
                            textarea {
                                value: "{raw}",
                                oninput: on_textarea_input,
                                rows: "10",
                                style: "width: 100%; font-family: monospace; font-size: 12px;",
                                placeholder: placeholder_text,
                            }
                            {error_render}
                            {write_error_line.clone()}
                            {summary_render}
                            div { class: "dialog__list",
                                {preview_items_render}
                                {preview_extra_line}
                            }
                        }
                        div { class: "dialog__footer",
                            button {
                                class: "btn btn--ghost btn--sm",
                                onclick: move |_| { cs.write().close_dialog(); },
                                "Cancel"
                            }
                            button {
                                class: "btn btn--sm",
                                onclick: on_parse_click,
                                "Parse"
                            }
                            button {
                                class: "btn btn--danger btn--sm",
                                disabled: !parse_ready,
                                onclick: confirm_delete.clone(),
                                "Delete {total} row(s)"
                            }
                        }
                    }
                }
            }
        }
        Some(DialogState::Compression {
            table,
            compressed,
            force,
            busy,
        }) => {
            // The dialog always offers the opposite of what the server reported.
            let target = !compressed;
            let current_label = if compressed {
                "compressed"
            } else {
                "not compressed"
            };
            let action_label = if target {
                "Enable compression"
            } else {
                "Disable compression"
            };
            let force_label = if target {
                "Compress the rows already in memory as well"
            } else {
                "Decompress the rows already in memory as well"
            };
            let apply_label = if busy { "Applying…" } else { action_label };
            let table_for_submit = table.clone();

            let on_force_toggle = move |evt: dioxus::events::FormEvent| {
                let checked = evt.checked();
                let mut w = cs.write();
                if let Some(DialogState::Compression { force, .. }) = w.dialog.as_mut() {
                    *force = checked;
                }
            };

            let on_apply = move |_| {
                let table = table_for_submit.clone();
                let force = {
                    let mut w = cs.write();
                    match w.dialog.as_mut() {
                        Some(DialogState::Compression { force, busy, .. }) => {
                            if *busy {
                                return;
                            }
                            *busy = true;
                            *force
                        }
                        _ => return,
                    }
                };
                spawn(async move {
                    match crate::api::update_table_compressed(&table, target, force).await {
                        Ok(()) => {
                            // The 3s status poll brings the new flag back on its
                            // own, so there is nothing to refresh here.
                            cs.write().close_dialog();
                        }
                        Err(err) => {
                            let mut w = cs.write();
                            if let Some(DialogState::Compression { busy, .. }) = w.dialog.as_mut() {
                                *busy = false;
                            }
                            w.set_write_error(err.to_string());
                        }
                    }
                });
            };

            rsx! {
                div { class: "dialog-overlay",
                    div { class: "dialog",
                        div { class: "dialog__header", "Table compression" }
                        div { class: "dialog__body",
                            div { style: "margin-bottom: 8px;",
                                "Table "
                                b { "{table}" }
                                " is currently "
                                b { "{current_label}" }
                                "."
                            }
                            label { style: "display: flex; align-items: center; gap: 8px; margin-top: 12px; font-size: 13px; cursor: pointer;",
                                input {
                                    r#type: "checkbox",
                                    checked: force,
                                    disabled: busy,
                                    onchange: on_force_toggle,
                                }
                                "{force_label}"
                            }
                            div { style: "margin-top: 6px; font-size: 12px; color: var(--text-muted);",
                                "Left unticked, the flag applies only to rows written from now on. Ticked, the whole table is re-encoded in place, which takes time proportional to its size."
                            }
                            {write_error_line.clone()}
                        }
                        div { class: "dialog__footer",
                            button {
                                class: "btn btn--ghost btn--sm",
                                disabled: busy,
                                onclick: move |_| { cs.write().close_dialog(); },
                                "Cancel"
                            }
                            button {
                                class: "btn btn--primary btn--sm",
                                disabled: busy,
                                onclick: on_apply,
                                "{apply_label}"
                            }
                        }
                    }
                }
            }
        }
        None => rsx! {},
    };

    let data_cls = if url_row.is_some() {
        "data"
    } else {
        "data data--no-drawer"
    };

    rsx! {
        section { class: "page page--flush",
            div { class: data_cls,
                TablesPane {
                    tables: tables.clone(),
                    selected: selected_table.clone(),
                    writer_tables,
                    metrics: table_metrics,
                    on_select: move |name| select_table(name),
                }
                {partitions_content}
                {center_content}
                {drawer_content}
            }
            {dialog_render}
            Outlet::<AppRoute> {}
        }
    }
}

/// A minimal row drawer used while rows are still loading or when the URL
/// points at a row key that no longer exists.
#[component]
fn DrawerMessage(title: String, message: String, on_close: EventHandler<()>) -> Element {
    rsx! {
        aside { class: "row-drawer",
            div { class: "row-drawer__header",
                span { class: "row-drawer__title", "Row Detail" }
                button {
                    class: "topbar__icon-btn",
                    onclick: move |_| on_close.call(()),
                    Icon { kind: IconKind::X }
                }
            }
            div { class: "row-drawer__body",
                div { class: "empty-state",
                    div { class: "empty-state__title", "{title}" }
                    div { class: "empty-state__sub", "{message}" }
                }
            }
        }
    }
}

fn parse_paste_delete_input(
    raw: &str,
) -> Result<(BTreeMap<String, Vec<String>>, usize, usize), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Input is empty.".to_string());
    }

    let parsed: Value =
        serde_json::from_str(trimmed).map_err(|err| format!("Invalid JSON: {}", err))?;

    let arr = parsed
        .as_array()
        .ok_or_else(|| "Top-level JSON must be an array.".to_string())?;

    if arr.is_empty() {
        return Err("Array is empty.".to_string());
    }

    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut total: usize = 0;

    for (idx, item) in arr.iter().enumerate() {
        let obj = item.as_object().ok_or_else(|| {
            format!(
                "Item #{} is not an object (expected {{ PartitionKey, RowKey }}).",
                idx
            )
        })?;

        let pk = obj
            .get(PARTITION_KEY)
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Item #{} is missing a string \"PartitionKey\" field.", idx))?;
        let rk = obj
            .get(ROW_KEY)
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Item #{} is missing a string \"RowKey\" field.", idx))?;

        if pk.is_empty() {
            return Err(format!("Item #{}: PartitionKey is empty.", idx));
        }
        if rk.is_empty() {
            return Err(format!("Item #{}: RowKey is empty.", idx));
        }

        grouped
            .entry(pk.to_string())
            .or_insert_with(Vec::new)
            .push(rk.to_string());
        total += 1;
    }

    let partitions_touched = grouped.len();
    Ok((grouped, total, partitions_touched))
}

fn build_rows_state(rows: Vec<Value>) -> (Vec<String>, Vec<Value>) {
    let mut headers: Vec<String> = vec![
        PARTITION_KEY.to_string(),
        ROW_KEY.to_string(),
        TIME_STAMP.to_string(),
    ];

    for row in rows.iter() {
        if let Value::Object(map) = row {
            for key in map.keys() {
                if key == PARTITION_KEY || key == ROW_KEY || key == TIME_STAMP {
                    continue;
                }
                if !headers.iter().any(|h| h == key) {
                    headers.push(key.clone());
                }
            }
        }
    }

    (headers, rows)
}

fn build_writer_tables(status: &Option<StatusApiModel>) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Some(s) = status {
        if let Some(init) = &s.initialized {
            for w in &init.writers {
                for t in &w.tables {
                    set.insert(t.clone());
                }
            }
        }
    }
    set
}

fn derive_table_connectivity(status: &Option<StatusApiModel>, table: &str) -> (Vec<String>, usize) {
    let mut writers = Vec::new();
    let mut readers = 0usize;
    if let Some(s) = status {
        if let Some(init) = &s.initialized {
            for w in &init.writers {
                if w.tables.iter().any(|t| t == table) {
                    writers.push(w.name.clone());
                }
            }
            for r in &init.readers {
                if r.is_node {
                    continue;
                }
                if r.tables.iter().any(|t| t == table) {
                    readers += 1;
                }
            }
        }
    }
    (writers, readers)
}

fn derive_table_stats(status: &Option<StatusApiModel>, table: &str) -> Option<TableApiModel> {
    status
        .as_ref()
        .and_then(|s| s.initialized.as_ref())
        .and_then(|init| init.tables.iter().find(|t| t.name == table).cloned())
}

fn derive_table_metrics(status: &Option<StatusApiModel>) -> HashMap<String, TableListMetrics> {
    let mut map = HashMap::new();
    if let Some(s) = status {
        if let Some(init) = &s.initialized {
            for t in &init.tables {
                map.insert(
                    t.name.clone(),
                    TableListMetrics {
                        partitions_count: t.partitions_count,
                        data_size: t.data_size,
                    },
                );
            }
        }
    }
    map
}

fn render_empty_state(
    tables: Vec<TableListItemApiModel>,
    on_pick: impl FnMut(String) + Clone + 'static,
) -> Element {
    let chips = tables.into_iter().take(8).map(|t| {
        let name = t.name.clone();
        let mut on_pick = on_pick.clone();
        rsx! {
            button {
                class: "btn btn--sm",
                onclick: move |_| on_pick(name.clone()),
                Badge { text: t.name.clone(), tone: BadgeTone::Neutral }
            }
        }
    });

    rsx! {
        div { class: "rows-col",
            div { class: "empty-state",
                div { class: "empty-state__icon",
                    Icon { kind: IconKind::Layers }
                }
                div { class: "empty-state__title", "Select a table to begin" }
                div { class: "empty-state__sub", "Choose a table from the left, or pick one of the recently active tables below." }
                div { class: "empty-state__chips", {chips} }
            }
        }
    }
}
