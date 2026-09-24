use std::sync::Arc;

use my_logger::LogEventCtx;

use crate::app::{AppContext, DbNamespace};

use super::utils::compile_backup_file;
use super::SnapshotFileModel;

/// `MaxBackupsToKeep` is counted inside each namespace: they back up
/// independently, so one busy namespace must not evict another one's snapshots.
/// Both the listing and the deletion resolve inside the namespace's own folder,
/// so this can not reach across namespaces.
pub async fn gc_backups(app: &AppContext) {
    for db_namespace in app.namespaces.get_all() {
        gc_namespace_backups(app, &db_namespace).await;
    }
}

/// Enforces `MaxBackupsToKeep` inside one namespace. Called right after a
/// snapshot is written — the new snapshot is what pushes the folder over the
/// limit, so collecting there keeps the limit true continuously instead of only
/// between the collector's ticks.
pub async fn gc_namespace_backups(app: &AppContext, db_namespace: &Arc<DbNamespace>) {
    let files = super::get_list_of_files(app, db_namespace).await;

    for file_name in select_backups_to_delete(&files, app.settings.max_backups_to_keep) {
        // Logged, not printed: whether MaxBackupsToKeep is being enforced at all
        // is the first question asked when a backup folder looks wrong, and the
        // answer belongs where the rest of the server's log is.
        my_logger::LOGGER.write_info(
            "GcBackups",
            format!("Deleting backup file {}", file_name),
            LogEventCtx::new()
                .add("namespace", db_namespace.name.to_string())
                .add("keeping", app.settings.max_backups_to_keep.to_string()),
        );
        delete_backup(app, db_namespace, file_name.as_str()).await;
    }
}

/// Names of the snapshots to drop: everything except the `max_to_keep` NEWEST.
///
/// Deliberately independent of the order it is handed: it re-derives "oldest"
/// from the timestamps rather than trusting a position in the vector. That is
/// the bug this replaced — the list arrived sorted by name ascending and the
/// collector popped the tail, which is the newest file, so every backup was
/// deleted within the same minute it was written while the five oldest ones
/// stayed forever.
fn select_backups_to_delete(files: &[SnapshotFileModel], max_to_keep: usize) -> Vec<String> {
    if files.len() <= max_to_keep {
        return Vec::new();
    }

    let mut oldest_first: Vec<&SnapshotFileModel> = files.iter().collect();

    oldest_first.sort_by(|left, right| {
        left.modified_unix_seconds
            .cmp(&right.modified_unix_seconds)
            .then_with(|| left.name.cmp(&right.name))
    });

    let to_delete = files.len() - max_to_keep;

    oldest_first
        .into_iter()
        .take(to_delete)
        .map(|itm| itm.name.clone())
        .collect()
}

async fn delete_backup(app: &AppContext, db_namespace: &Arc<DbNamespace>, file_name: &str) {
    let file_full_path = compile_backup_file(app, &db_namespace.name, file_name);

    if let Err(err) = tokio::fs::remove_file(file_full_path.as_str()).await {
        // Already gone is this function's own goal, not a failure: a namespace
        // collects right after writing its snapshot while the GcBackups timer
        // walks the same folder as a separate task, so the two do race — and
        // whoever gets there second must not report an error over a file the
        // other one has just deleted.
        if err.kind() == std::io::ErrorKind::NotFound {
            return;
        }

        // A snapshot that can not be deleted is not worth taking the server
        // down for: it is reported and stays in the folder, and the next tick
        // tries again.
        my_logger::LOGGER.write_error(
            "GcBackups",
            format!("Can not delete backup file. Err: {}", err),
            LogEventCtx::new().add("fileName", file_full_path),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, modified_unix_seconds: i64) -> SnapshotFileModel {
        SnapshotFileModel {
            name: name.to_string(),
            size: 1024,
            modified_unix_seconds,
        }
    }

    #[test]
    fn test_the_newest_are_kept() {
        // Six snapshots, keep five — the single oldest one has to go.
        let files = vec![
            file("20260731T160000.zip", 600),
            file("20260731T150000.zip", 500),
            file("20260731T140000.zip", 400),
            file("20260731T130000.zip", 300),
            file("20260731T120000.zip", 200),
            file("20260731T110000.zip", 100),
        ];

        let to_delete = select_backups_to_delete(&files, 5);

        assert_eq!(vec!["20260731T110000.zip".to_string()], to_delete);
    }

    #[test]
    fn test_freshly_written_snapshot_survives() {
        // The exact production scenario: the newest file is the one just
        // written by save_backup in this very tick. It must never be picked.
        let files = vec![
            file("20260731T140444.zip", 1000),
            file("20260627T140430.zip", 10),
            file("20260628T140430.zip", 20),
            file("20260629T140430.zip", 30),
            file("20260630T140430.zip", 40),
            file("20260630T150430.zip", 50),
        ];

        let to_delete = select_backups_to_delete(&files, 5);

        assert_eq!(vec!["20260627T140430.zip".to_string()], to_delete);
        assert_eq!(
            false,
            to_delete.contains(&"20260731T140444.zip".to_string())
        );
    }

    #[test]
    fn test_mixed_name_formats_delete_by_time_not_by_name() {
        // The old-format names sort BEFORE the new-format ones lexicographically
        // ('-' < '2') while being the NEWER files here. Sorting by name would
        // delete the new-format files; sorting by time keeps them.
        let files = vec![
            file("2026-06-27T07_09_54.zip", 900),
            file("2026-06-28T07_09_54.zip", 1000),
            file("20260731T185201.zip", 100),
            file("20260731T185202.zip", 200),
        ];

        let to_delete = select_backups_to_delete(&files, 2);

        assert_eq!(
            vec![
                "20260731T185201.zip".to_string(),
                "20260731T185202.zip".to_string()
            ],
            to_delete
        );
    }

    #[test]
    fn test_backlog_is_collected_in_one_go() {
        // After the fix a production folder holds a pile of stuck old files.
        // The first tick has to clear all of them at once, not one per tick.
        let files: Vec<SnapshotFileModel> = (0..20)
            .map(|i| file(format!("2026073{}T110000.zip", i % 10).as_str(), i as i64))
            .collect();

        let to_delete = select_backups_to_delete(&files, 5);

        assert_eq!(15, to_delete.len());
    }

    #[test]
    fn test_nothing_to_delete_when_under_the_limit() {
        let files = vec![
            file("20260731T120000.zip", 100),
            file("20260731T130000.zip", 200),
        ];

        assert_eq!(0, select_backups_to_delete(&files, 5).len());
        assert_eq!(0, select_backups_to_delete(&files, 2).len());
        assert_eq!(0, select_backups_to_delete(&[], 5).len());
    }

    #[test]
    fn test_same_second_snapshots_are_ordered_by_name() {
        // Two files written within the same second: the tie-breaker keeps the
        // choice deterministic instead of depending on directory order.
        let files = vec![
            file("20260731T120001.zip", 100),
            file("20260731T120002.zip", 100),
        ];

        assert_eq!(
            vec!["20260731T120001.zip".to_string()],
            select_backups_to_delete(&files, 1)
        );
    }
}
