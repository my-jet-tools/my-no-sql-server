use std::collections::HashSet;

use dioxus::prelude::*;
use serde_json::Value;

use crate::AppRoute;
use crate::api;
use crate::components::data::{PARTITION_KEY, ROW_KEY, RowsTable, TIME_STAMP};
use crate::models::{SnapshotFileApiModel, SnapshotTableApiModel};

#[derive(Default)]
struct SnapshotsState {
    // Snapshot files list (loaded once, refreshed on demand).
    files: Vec<SnapshotFileApiModel>,
    files_started: bool,
    files_ready: bool,

    // Tables for the selected file.
    loaded_for_file: Option<String>,
    tables: Vec<SnapshotTableApiModel>,
    tables_ready: bool,
    // Snapshot tables ticked for a bulk restore (scoped to the current file).
    checked_tables: HashSet<String>,

    // Partitions for the selected (file, table).
    loaded_for_table: Option<(String, String)>,
    partitions: Vec<String>,
    partitions_ready: bool,

    // Rows for the selected (file, table, partition).
    loaded_rows_for: Option<(String, String, String)>,
    row_headers: Vec<String>,
    rows: Vec<Value>,
    rows_ready: bool,

    // Active "restore table from backup" dialog, if any.
    restore: Option<RestoreDialog>,

    // True while a forced "make snapshot" request is in flight.
    making_snapshot: bool,

    error: Option<String>,
}

/// State of the confirmation dialog shown when restoring from a snapshot.
///
/// `tables` holds one or more whole tables to restore (a single entry for the
/// per-row "Restore" button, several for a bulk restore). When `partition` is
/// `Some`, it is always a single-table restore of just that partition's rows.
struct RestoreDialog {
    tables: Vec<String>,
    partition: Option<String>,
    clean_table: bool,
    in_progress: bool,
    /// How many of `tables` have been restored so far (bulk progress).
    completed: usize,
    error: Option<String>,
    done: bool,
}

impl SnapshotsState {
    fn begin_files_load(&mut self) {
        self.files_started = true;
        self.files_ready = false;
        self.error = None;
    }

    fn set_files(&mut self, files: Vec<SnapshotFileApiModel>) {
        self.files = files;
        self.files_ready = true;
    }

    fn begin_make_snapshot(&mut self) {
        self.making_snapshot = true;
        self.error = None;
    }

    fn make_snapshot_done(&mut self) {
        self.making_snapshot = false;
        // Force the files list to reload so the freshly created snapshot shows up.
        self.files_started = false;
    }

    fn make_snapshot_error(&mut self, err: String) {
        self.making_snapshot = false;
        self.error = Some(err);
    }

    fn begin_tables_load(&mut self, file: &str) {
        self.loaded_for_file = Some(file.to_string());
        self.tables = Vec::new();
        self.tables_ready = false;
        self.checked_tables.clear();
        self.error = None;
    }

    fn toggle_table_check(&mut self, table: &str) {
        if !self.checked_tables.remove(table) {
            self.checked_tables.insert(table.to_string());
        }
    }

    fn toggle_all_tables_check(&mut self, check_all: bool) {
        if check_all {
            self.checked_tables = self.tables.iter().map(|t| t.name.clone()).collect();
        } else {
            self.checked_tables.clear();
        }
    }

    fn clear_table_checks(&mut self) {
        self.checked_tables.clear();
    }

    /// Selected table names in the order they appear in `tables`.
    fn selected_tables(&self) -> Vec<String> {
        self.tables
            .iter()
            .map(|t| t.name.clone())
            .filter(|n| self.checked_tables.contains(n))
            .collect()
    }

    fn set_tables(&mut self, file: &str, tables: Vec<SnapshotTableApiModel>) {
        if self.loaded_for_file.as_deref() == Some(file) {
            self.tables = tables;
            self.tables_ready = true;
        }
    }

    fn begin_partitions_load(&mut self, key: &(String, String)) {
        self.loaded_for_table = Some(key.clone());
        self.partitions = Vec::new();
        self.partitions_ready = false;
        self.error = None;
    }

    fn set_partitions(&mut self, key: &(String, String), partitions: Vec<String>) {
        if self.loaded_for_table.as_ref() == Some(key) {
            self.partitions = partitions;
            self.partitions_ready = true;
        }
    }

    fn begin_rows_load(&mut self, key: &(String, String, String)) {
        self.loaded_rows_for = Some(key.clone());
        self.row_headers = Vec::new();
        self.rows = Vec::new();
        self.rows_ready = false;
        self.error = None;
    }

    fn set_rows(&mut self, key: &(String, String, String), headers: Vec<String>, rows: Vec<Value>) {
        if self.loaded_rows_for.as_ref() == Some(key) {
            self.row_headers = headers;
            self.rows = rows;
            self.rows_ready = true;
        }
    }

    fn open_restore(&mut self, tables: Vec<String>) {
        self.restore = Some(RestoreDialog {
            tables,
            partition: None,
            clean_table: false,
            in_progress: false,
            completed: 0,
            error: None,
            done: false,
        });
    }

    fn open_restore_partition(&mut self, table: String, partition: String) {
        self.restore = Some(RestoreDialog {
            tables: vec![table],
            partition: Some(partition),
            clean_table: false,
            in_progress: false,
            completed: 0,
            error: None,
            done: false,
        });
    }

    fn set_clean_table(&mut self, value: bool) {
        if let Some(dialog) = self.restore.as_mut() {
            dialog.clean_table = value;
        }
    }

    fn restore_begin(&mut self) {
        if let Some(dialog) = self.restore.as_mut() {
            dialog.in_progress = true;
            dialog.completed = 0;
            dialog.error = None;
        }
    }

    fn restore_set_completed(&mut self, completed: usize) {
        if let Some(dialog) = self.restore.as_mut() {
            dialog.completed = completed;
        }
    }

    fn restore_done(&mut self) {
        if let Some(dialog) = self.restore.as_mut() {
            dialog.in_progress = false;
            dialog.done = true;
        }
        // Restored tables are no longer "pending" — drop the selection so the
        // checkboxes are cleared once the dialog closes.
        self.checked_tables.clear();
    }

    fn restore_error(&mut self, err: String) {
        if let Some(dialog) = self.restore.as_mut() {
            dialog.in_progress = false;
            dialog.error = Some(err);
        }
    }

    fn close_restore(&mut self) {
        self.restore = None;
    }
}

/// Extract `(file, table, partition)` from the current snapshot route.
fn parse_snapshot_route(route: &AppRoute) -> (Option<String>, Option<String>, Option<String>) {
    match route {
        AppRoute::SnapshotFile { file } => (Some(file.clone()), None, None),
        AppRoute::SnapshotTable { file, table } => (Some(file.clone()), Some(table.clone()), None),
        AppRoute::SnapshotPartition {
            file,
            table,
            partition,
        } => (
            Some(file.clone()),
            Some(table.clone()),
            Some(partition.clone()),
        ),
        _ => (None, None, None),
    }
}

// Route placeholders — the URL patterns for the snapshots section. `SnapshotsLayout`
// renders the whole page and reads the params via `use_route`, so these render
// nothing themselves.
#[component]
pub fn Snapshots() -> Element {
    rsx! {}
}

#[component]
pub fn SnapshotFile(file: String) -> Element {
    let _ = file;
    rsx! {}
}

#[component]
pub fn SnapshotTable(file: String, table: String) -> Element {
    let _ = (file, table);
    rsx! {}
}

#[component]
pub fn SnapshotPartition(file: String, table: String, partition: String) -> Element {
    let _ = (file, table, partition);
    rsx! {}
}

#[component]
pub fn SnapshotsLayout() -> Element {
    let mut cs = use_signal(SnapshotsState::default);
    let nav = navigator();

    let route = use_route::<AppRoute>();
    let (url_file, url_table, url_partition) = parse_snapshot_route(&route);

    // ---- load files list ----
    if !cs.read().files_started {
        spawn(async move {
            if cs.peek().files_started {
                return;
            }
            cs.write().begin_files_load();
            match api::get_snapshots_list().await {
                Ok(mut files) => {
                    files.sort_by(|a, b| b.name.cmp(&a.name));
                    cs.write().set_files(files);
                }
                Err(err) => {
                    cs.write().error = Some(format!("Failed to load snapshots: {}", err));
                }
            }
        });
    }

    // ---- load tables whenever the URL file changes ----
    if let Some(file) = url_file.clone() {
        if cs.read().loaded_for_file.as_deref() != Some(file.as_str()) {
            spawn(async move {
                if cs.peek().loaded_for_file.as_deref() == Some(file.as_str()) {
                    return;
                }
                cs.write().begin_tables_load(&file);
                match api::get_snapshot_tables(&file).await {
                    Ok(tables) => cs.write().set_tables(&file, tables),
                    Err(err) => {
                        cs.write().error = Some(format!("Failed to load tables: {}", err));
                    }
                }
            });
        }
    }

    // ---- load partitions whenever the URL (file, table) changes ----
    if let (Some(file), Some(table)) = (url_file.clone(), url_table.clone()) {
        let key = (file, table);
        if cs.read().loaded_for_table.as_ref() != Some(&key) {
            spawn(async move {
                if cs.peek().loaded_for_table.as_ref() == Some(&key) {
                    return;
                }
                cs.write().begin_partitions_load(&key);
                match api::get_snapshot_partitions(&key.0, &key.1).await {
                    Ok(partitions) => cs.write().set_partitions(&key, partitions),
                    Err(err) => {
                        cs.write().error = Some(format!("Failed to load partitions: {}", err));
                    }
                }
            });
        }
    }

    // ---- load rows whenever the URL (file, table, partition) changes ----
    if let (Some(file), Some(table), Some(pk)) =
        (url_file.clone(), url_table.clone(), url_partition.clone())
    {
        let key = (file, table, pk);
        if cs.read().loaded_rows_for.as_ref() != Some(&key) {
            spawn(async move {
                if cs.peek().loaded_rows_for.as_ref() == Some(&key) {
                    return;
                }
                cs.write().begin_rows_load(&key);
                match api::get_snapshot_rows(&key.0, &key.1, &key.2).await {
                    Ok(rows) => {
                        let (headers, rows) = build_rows_state(rows);
                        cs.write().set_rows(&key, headers, rows);
                    }
                    Err(err) => {
                        cs.write().error = Some(format!("Failed to load rows: {}", err));
                    }
                }
            });
        }
    }

    // ---- snapshot of state for rendering ----
    let cs_ra = cs.read();
    let error = cs_ra.error.clone();
    let files = cs_ra.files.clone();
    let files_ready = cs_ra.files_ready;
    let making_snapshot = cs_ra.making_snapshot;

    let tables_scope =
        url_file.is_some() && cs_ra.loaded_for_file.as_deref() == url_file.as_deref();
    let tables = if tables_scope {
        cs_ra.tables.clone()
    } else {
        Vec::new()
    };
    let tables_ready = tables_scope && cs_ra.tables_ready;
    let checked_tables = if tables_scope {
        cs_ra.checked_tables.clone()
    } else {
        HashSet::new()
    };

    let partitions_key = match (&url_file, &url_table) {
        (Some(f), Some(t)) => Some((f.clone(), t.clone())),
        _ => None,
    };
    let partitions_scope =
        partitions_key.is_some() && cs_ra.loaded_for_table.as_ref() == partitions_key.as_ref();
    let partitions = if partitions_scope {
        cs_ra.partitions.clone()
    } else {
        Vec::new()
    };
    let partitions_ready = partitions_scope && cs_ra.partitions_ready;

    let rows_key = match (&url_file, &url_table, &url_partition) {
        (Some(f), Some(t), Some(p)) => Some((f.clone(), t.clone(), p.clone())),
        _ => None,
    };
    let rows_scope = rows_key.is_some() && cs_ra.loaded_rows_for.as_ref() == rows_key.as_ref();
    let row_headers = if rows_scope {
        cs_ra.row_headers.clone()
    } else {
        Vec::new()
    };
    let rows = if rows_scope {
        cs_ra.rows.clone()
    } else {
        Vec::new()
    };
    let rows_ready = rows_scope && cs_ra.rows_ready;
    drop(cs_ra);

    // Refresh re-fetches the deepest level matching the current URL by clearing
    // its load marker — the inline loaders above pick it up on the next render.
    let refresh_file = url_file.clone();
    let refresh_table = url_table.clone();
    let refresh_partition = url_partition.clone();
    let on_refresh = move |_| {
        let mut w = cs.write();
        match (
            refresh_file.is_some(),
            refresh_table.is_some(),
            refresh_partition.is_some(),
        ) {
            (false, _, _) => w.files_started = false,
            (true, false, _) => w.loaded_for_file = None,
            (true, true, false) => w.loaded_for_table = None,
            (true, true, true) => w.loaded_rows_for = None,
        }
    };

    // Force-create a snapshot on the server, then reload the files list.
    let on_make_snapshot = move |_| {
        if cs.peek().making_snapshot {
            return;
        }
        cs.write().begin_make_snapshot();
        spawn(async move {
            match api::make_snapshot().await {
                Ok(_) => cs.write().make_snapshot_done(),
                Err(err) => cs
                    .write()
                    .make_snapshot_error(format!("Failed to make snapshot: {}", err)),
            }
        });
    };

    let loading = match (&url_file, &url_table, &url_partition) {
        (None, _, _) => !files_ready,
        (Some(_), None, _) => !tables_ready,
        (Some(_), Some(_), None) => !partitions_ready,
        (Some(_), Some(_), Some(_)) => !rows_ready,
    };

    // ---- navigation handlers ----
    let open_file = move |file: String| {
        nav.push(AppRoute::SnapshotFile { file });
    };
    let open_table = {
        let file = url_file.clone();
        move |table: String| {
            if let Some(file) = file.clone() {
                nav.push(AppRoute::SnapshotTable { file, table });
            }
        }
    };
    let open_partition = {
        let file = url_file.clone();
        let table = url_table.clone();
        move |partition: String| {
            if let (Some(file), Some(table)) = (file.clone(), table.clone()) {
                nav.push(AppRoute::SnapshotPartition {
                    file,
                    table,
                    partition,
                });
            }
        }
    };

    // ---- table selection (checkboxes for bulk restore) ----
    let on_toggle_table = move |table: String| {
        cs.write().toggle_table_check(&table);
    };
    let on_toggle_all_tables = move |check_all: bool| {
        cs.write().toggle_all_tables_check(check_all);
    };
    let clear_table_checks = move |_| {
        cs.write().clear_table_checks();
    };

    // ---- restore dialog handlers (table(s) or single partition) ----
    let open_restore = move |table: String| {
        cs.write().open_restore(vec![table]);
    };
    let open_restore_selected = move |_| {
        let selected = cs.read().selected_tables();
        if !selected.is_empty() {
            cs.write().open_restore(selected);
        }
    };
    let open_restore_partition = {
        let table = url_table.clone();
        move |partition: String| {
            if let Some(table) = table.clone() {
                cs.write().open_restore_partition(table, partition);
            }
        }
    };
    let confirm_restore = {
        let file = url_file.clone();
        move |_| {
            let (file, tables, partition, clean) = {
                let ra = cs.read();
                match (file.as_ref(), ra.restore.as_ref()) {
                    (Some(f), Some(dialog)) if !dialog.in_progress && !dialog.done => (
                        f.clone(),
                        dialog.tables.clone(),
                        dialog.partition.clone(),
                        dialog.clean_table,
                    ),
                    _ => return,
                }
            };
            cs.write().restore_begin();
            spawn(async move {
                match partition {
                    // Single-partition restore — always exactly one table.
                    Some(partition) => {
                        let table = tables.first().cloned().unwrap_or_default();
                        match api::restore_partition_from_backup(&file, &table, &partition).await {
                            Ok(_) => cs.write().restore_done(),
                            Err(err) => cs.write().restore_error(err.to_string()),
                        }
                    }
                    // One or more whole tables — restore sequentially so a
                    // failure names the offending table and stops the rest.
                    None => {
                        for (idx, table) in tables.iter().enumerate() {
                            match api::restore_table_from_backup(&file, table, clean).await {
                                Ok(_) => cs.write().restore_set_completed(idx + 1),
                                Err(err) => {
                                    cs.write()
                                        .restore_error(format!("Table '{}': {}", table, err));
                                    return;
                                }
                            }
                        }
                        cs.write().restore_done();
                    }
                }
            });
        }
    };

    let go_to_files = move |_| {
        nav.push(AppRoute::Snapshots {});
    };
    let go_to_tables = {
        let file = url_file.clone();
        move |_| {
            if let Some(file) = file.clone() {
                nav.push(AppRoute::SnapshotFile { file });
            }
        }
    };
    let go_to_partitions = {
        let file = url_file.clone();
        let table = url_table.clone();
        move |_| {
            if let (Some(file), Some(table)) = (file.clone(), table.clone()) {
                nav.push(AppRoute::SnapshotTable { file, table });
            }
        }
    };

    let error_view = if let Some(err) = error {
        rsx! {
            div { style: "color: var(--danger); font-size: 12.5px; padding: 8px 0;", "{err}" }
        }
    } else {
        rsx! {}
    };

    let body = match (url_file.clone(), url_table.clone(), url_partition.clone()) {
        (None, _, _) => render_files(files, !files_ready, open_file),
        (Some(_), None, _) => render_tables(
            tables,
            !tables_ready,
            checked_tables,
            open_table,
            open_restore,
            on_toggle_table,
            on_toggle_all_tables,
            clear_table_checks,
            open_restore_selected,
        ),
        (Some(_), Some(_), None) => render_partitions(
            partitions,
            !partitions_ready,
            open_partition,
            open_restore_partition,
        ),
        (Some(_), Some(_), Some(_)) => render_rows(row_headers, rows, !rows_ready),
    };

    let crumbs = render_crumbs(
        url_file.clone(),
        url_table.clone(),
        url_partition.clone(),
        go_to_files,
        go_to_tables,
        go_to_partitions,
    );

    let restore_file_name = url_file.clone().unwrap_or_default();
    let restore_render = render_restore_dialog(cs, &restore_file_name, confirm_restore);

    rsx! {
        section { class: "page page--padded",
            div { style: "display: flex; flex-direction: column; gap: 14px; max-width: 960px;",
                div { style: "display: flex; align-items: center; justify-content: space-between; gap: 12px;",
                    {crumbs}
                    div { style: "display: flex; align-items: center; gap: 8px;",
                        button {
                            class: "btn btn--primary btn--sm",
                            disabled: making_snapshot,
                            onclick: on_make_snapshot,
                            if making_snapshot { "Making snapshot…" } else { "Make snapshot" }
                        }
                        button {
                            class: "btn btn--ghost btn--sm",
                            disabled: loading,
                            onclick: on_refresh,
                            if loading { "Refreshing…" } else { "Refresh" }
                        }
                    }
                }
                {error_view}
                {body}
            }
        }
        {restore_render}
        Outlet::<AppRoute> {}
    }
}

/// Renders the "restore table from backup" confirmation dialog (or nothing when
/// no restore is in progress). `confirm` triggers the actual restore request.
fn render_restore_dialog(
    mut cs: Signal<SnapshotsState>,
    file_name: &str,
    confirm: impl FnMut(()) + Clone + 'static,
) -> Element {
    let ra = cs.read();
    let Some(dialog) = ra.restore.as_ref() else {
        return rsx! {};
    };

    let tables = dialog.tables.clone();
    let partition = dialog.partition.clone();
    let clean_table = dialog.clean_table;
    let in_progress = dialog.in_progress;
    let completed = dialog.completed;
    let done = dialog.done;
    let error = dialog.error.clone();
    drop(ra);

    let total = tables.len();
    let multi = partition.is_none() && total > 1;
    // First table — used for the single-table / partition wording.
    let first_table = tables.first().cloned().unwrap_or_default();
    // Bullet list of table names, shown only for a multi-table restore.
    let tables_list = if multi {
        let items = tables.iter().map(|t| {
            rsx! {
                li { key: "{t}", style: "font-family: var(--font-mono);", "{t}" }
            }
        });
        rsx! {
            ul { style: "margin: 8px 0 0; padding-left: 20px; font-size: 12.5px; max-height: 180px; overflow: auto;",
                {items}
            }
        }
    } else {
        rsx! {}
    };

    let file_name = file_name.to_string();
    let mut confirm = confirm;

    let error_view = if let Some(err) = error {
        rsx! {
            div { style: "color: var(--danger); font-size: 12.5px; margin-top: 10px;", "{err}" }
        }
    } else {
        rsx! {}
    };

    // Clean-table checkbox only applies to a whole-table restore. Restoring a
    // single partition always replaces just that partition's rows.
    let clean_checkbox = if partition.is_none() {
        rsx! {
            label { style: "display: flex; align-items: center; gap: 8px; margin-top: 12px; font-size: 13px; cursor: pointer;",
                input {
                    r#type: "checkbox",
                    checked: clean_table,
                    disabled: in_progress,
                    onchange: move |evt| { cs.write().set_clean_table(evt.checked()); },
                }
                "Clean table before restore (delete existing rows)"
            }
        }
    } else {
        rsx! {}
    };

    let body = if done {
        let what = match (&partition, multi) {
            (Some(pk), _) => rsx! {
                "Partition "
                b { "{pk}" }
                " of table "
                b { "{first_table}" }
            },
            (None, true) => rsx! {
                b { "{total}" }
                " tables"
            },
            (None, false) => rsx! {
                "Table "
                b { "{first_table}" }
            },
        };
        let restored_verb = if multi {
            " have been restored from "
        } else {
            " has been restored from "
        };
        rsx! {
            div { class: "dialog__body",
                {what}
                {restored_verb}
                b { "{file_name}" }
                "."
                {tables_list}
            }
            div { class: "dialog__footer",
                button {
                    class: "btn btn--primary btn--sm",
                    onclick: move |_| { cs.write().close_restore(); },
                    "Close"
                }
            }
        }
    } else {
        let what = match (&partition, multi) {
            (Some(pk), _) => rsx! {
                "Restore partition "
                b { "{pk}" }
                " of table "
                b { "{first_table}" }
            },
            (None, true) => rsx! {
                "Restore "
                b { "{total}" }
                " selected tables"
            },
            (None, false) => rsx! {
                "Restore table "
                b { "{first_table}" }
            },
        };
        let restore_label = if in_progress {
            if multi {
                format!("Restoring… ({}/{})", completed, total)
            } else {
                "Restoring…".to_string()
            }
        } else {
            "Restore".to_string()
        };
        rsx! {
            div { class: "dialog__body",
                {what}
                " from snapshot "
                b { "{file_name}" }
                "?"
                {tables_list}
                {clean_checkbox}
                {error_view}
            }
            div { class: "dialog__footer",
                button {
                    class: "btn btn--ghost btn--sm",
                    disabled: in_progress,
                    onclick: move |_| { cs.write().close_restore(); },
                    "Cancel"
                }
                button {
                    class: "btn btn--primary btn--sm",
                    disabled: in_progress,
                    onclick: move |_| confirm(()),
                    "{restore_label}"
                }
            }
        }
    };

    rsx! {
        div { class: "dialog-overlay",
            div { class: "dialog",
                div { class: "dialog__header", "Restore from backup" }
                {body}
            }
        }
    }
}

fn render_crumbs(
    file: Option<String>,
    table: Option<String>,
    partition: Option<String>,
    on_root: impl FnMut(()) + Clone + 'static,
    on_file: impl FnMut(()) + Clone + 'static,
    on_table: impl FnMut(()) + Clone + 'static,
) -> Element {
    let mut on_root = on_root;
    let mut on_file = on_file;
    let mut on_table = on_table;

    let file_segment = file.clone();
    let table_segment = table.clone();
    let partition_segment = partition.clone();

    let file_active = file.is_some();
    let table_active = table.is_some();
    let partition_active = partition.is_some();

    rsx! {
        div { style: "display: flex; flex-wrap: wrap; align-items: center; gap: 6px; font-size: 13px; color: var(--text-muted);",
            button {
                class: "btn btn--ghost btn--sm",
                disabled: !file_active,
                onclick: move |_| on_root(()),
                "Snapshots"
            }
            if let Some(name) = file_segment {
                span { "›" }
                button {
                    class: "btn btn--ghost btn--sm",
                    style: "font-family: var(--font-mono);",
                    disabled: !table_active,
                    onclick: move |_| on_file(()),
                    "{name}"
                }
            }
            if let Some(name) = table_segment {
                span { "›" }
                button {
                    class: "btn btn--ghost btn--sm",
                    style: "font-family: var(--font-mono);",
                    disabled: !partition_active,
                    onclick: move |_| on_table(()),
                    "{name}"
                }
            }
            if let Some(name) = partition_segment {
                span { "›" }
                span { style: "font-family: var(--font-mono); padding: 0 6px;", "{name}" }
            }
        }
    }
}

fn render_files(
    files: Vec<SnapshotFileApiModel>,
    loading: bool,
    on_pick: impl FnMut(String) + Clone + 'static,
) -> Element {
    if loading && files.is_empty() {
        return rsx! {
            div { class: "empty-state",
                div { class: "empty-state__title", "Loading snapshots…" }
            }
        };
    }
    if files.is_empty() {
        return rsx! {
            div { class: "empty-state",
                div { class: "empty-state__title", "No snapshots yet" }
                div { class: "empty-state__sub",
                    "Snapshots are written periodically into the backup folder configured on the server."
                }
            }
        };
    }
    let count = files.len();
    let rows = files.into_iter().map(move |file| {
        let n = file.name.clone();
        let size = crate::utils::format_bytes(file.size as f64);
        let mut on_pick = on_pick.clone();
        rsx! {
            tr {
                key: "{file.name}",
                style: "cursor: pointer;",
                onclick: move |_| on_pick(n.clone()),
                td { style: "font-family: var(--font-mono);", "{file.name}" }
                td { class: "num", "{size}" }
            }
        }
    });
    rsx! {
        div { class: "card",
            div { class: "card__header",
                span { class: "card__title", "Snapshot files" }
                span { class: "card__subtitle", "{count} file(s)" }
            }
            div { class: "card__body",
                table { class: "rt",
                    thead {
                        tr {
                            th { "File name" }
                            th { class: "num", "Size" }
                        }
                    }
                    tbody { {rows} }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_tables(
    tables: Vec<SnapshotTableApiModel>,
    loading: bool,
    checked: HashSet<String>,
    on_pick: impl FnMut(String) + Clone + 'static,
    on_restore: impl FnMut(String) + Clone + 'static,
    on_toggle: impl FnMut(String) + Clone + 'static,
    on_toggle_all: impl FnMut(bool) + Clone + 'static,
    on_clear: impl FnMut(()) + Clone + 'static,
    on_restore_selected: impl FnMut(()) + Clone + 'static,
) -> Element {
    if loading && tables.is_empty() {
        return rsx! {
            div { class: "empty-state",
                div { class: "empty-state__title", "Loading tables…" }
            }
        };
    }
    if tables.is_empty() {
        return rsx! {
            div { class: "empty-state",
                div { class: "empty-state__title", "No tables in this snapshot" }
            }
        };
    }
    let count = tables.len();
    let selected_count = tables.iter().filter(|t| checked.contains(&t.name)).count();
    let all_checked = selected_count == count && count > 0;

    let rows = tables.into_iter().map(move |t| {
        let n = t.name.clone();
        let restore_name = t.name.clone();
        let toggle_name = t.name.clone();
        let is_checked = checked.contains(&t.name);
        let mut on_pick = on_pick.clone();
        let mut on_restore = on_restore.clone();
        let mut on_toggle = on_toggle.clone();
        rsx! {
            tr {
                key: "{t.name}",
                style: "cursor: pointer;",
                onclick: move |_| on_pick(n.clone()),
                td {
                    class: "rt-check",
                    onclick: move |evt| {
                        evt.stop_propagation();
                        on_toggle(toggle_name.clone());
                    },
                    input { r#type: "checkbox", checked: is_checked }
                }
                td { style: "font-family: var(--font-mono);", "{t.name}" }
                td { class: "num", "{t.partitions_count}" }
                td {
                    class: "num",
                    onclick: move |evt| { evt.stop_propagation(); },
                    button {
                        class: "btn btn--ghost btn--sm",
                        onclick: move |evt| {
                            evt.stop_propagation();
                            on_restore(restore_name.clone());
                        },
                        "Restore"
                    }
                }
            }
        }
    });

    let bulk_bar = if selected_count > 0 {
        let mut on_clear = on_clear.clone();
        let mut on_restore_selected = on_restore_selected.clone();
        rsx! {
            div { class: "bulk-bar",
                span { class: "bulk-bar__count", "{selected_count} selected" }
                div { class: "bulk-bar__spacer" }
                button {
                    class: "btn btn--ghost btn--sm",
                    onclick: move |_| on_clear(()),
                    "Clear"
                }
                button {
                    class: "btn btn--primary btn--sm",
                    onclick: move |_| on_restore_selected(()),
                    "Restore selected"
                }
            }
        }
    } else {
        rsx! {}
    };

    let mut on_toggle_all = on_toggle_all.clone();
    rsx! {
        div { class: "card",
            div { class: "card__header",
                span { class: "card__title", "Tables" }
                span { class: "card__subtitle", "{count} table(s)" }
            }
            {bulk_bar}
            div { class: "card__body",
                table { class: "rt",
                    thead {
                        tr {
                            th {
                                class: "rt-check",
                                onclick: move |_| on_toggle_all(!all_checked),
                                input { r#type: "checkbox", checked: all_checked }
                            }
                            th { "Table name" }
                            th { class: "num", "Partitions" }
                            th { class: "num", "" }
                        }
                    }
                    tbody { {rows} }
                }
            }
        }
    }
}

fn render_partitions(
    partitions: Vec<String>,
    loading: bool,
    on_pick: impl FnMut(String) + Clone + 'static,
    on_restore: impl FnMut(String) + Clone + 'static,
) -> Element {
    if loading && partitions.is_empty() {
        return rsx! {
            div { class: "empty-state",
                div { class: "empty-state__title", "Loading partitions…" }
            }
        };
    }
    if partitions.is_empty() {
        return rsx! {
            div { class: "empty-state",
                div { class: "empty-state__title", "No partitions in this table" }
            }
        };
    }
    let count = partitions.len();
    let rows = partitions.into_iter().map(move |pk| {
        let p = pk.clone();
        let restore_pk = pk.clone();
        let mut on_pick = on_pick.clone();
        let mut on_restore = on_restore.clone();
        rsx! {
            tr {
                key: "{pk}",
                style: "cursor: pointer;",
                onclick: move |_| on_pick(p.clone()),
                td { style: "font-family: var(--font-mono);", "{pk}" }
                td {
                    class: "num",
                    onclick: move |evt| { evt.stop_propagation(); },
                    button {
                        class: "btn btn--ghost btn--sm",
                        onclick: move |evt| {
                            evt.stop_propagation();
                            on_restore(restore_pk.clone());
                        },
                        "Restore"
                    }
                }
            }
        }
    });
    rsx! {
        div { class: "card",
            div { class: "card__header",
                span { class: "card__title", "Partitions" }
                span { class: "card__subtitle", "{count} partition(s)" }
            }
            div { class: "card__body",
                table { class: "rt",
                    thead {
                        tr {
                            th { "Partition key" }
                            th { class: "num", "" }
                        }
                    }
                    tbody { {rows} }
                }
            }
        }
    }
}

fn render_rows(headers: Vec<String>, rows: Vec<Value>, loading: bool) -> Element {
    if loading && rows.is_empty() {
        return rsx! {
            div { class: "empty-state",
                div { class: "empty-state__title", "Loading rows…" }
            }
        };
    }
    let count = rows.len();
    rsx! {
        div { class: "card",
            div { class: "card__header",
                span { class: "card__title", "Rows" }
                span { class: "card__subtitle", "{count} row(s)" }
            }
            div { class: "card__body", style: "padding: 0;",
                RowsTable {
                    headers,
                    rows,
                    selected_row_key: None::<String>,
                    on_row_click: |_| {},
                }
            }
        }
    }
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
