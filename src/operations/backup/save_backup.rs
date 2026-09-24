use my_logger::LogEventCtx;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::app::AppContext;

use super::utils::*;
use super::SnapshotFileModel;

use std::sync::Arc;

use crate::app::DbNamespace;

/// The snapshot itself is missing: it was never built, or it never reached the
/// disk. The next tick retries it.
const SNAPSHOT_IS_MISSING: &str = "no snapshot reached the disk";

/// The snapshot part went through, but nothing on the disk says when. Reported
/// separately: the folder is not in the state it looks like it is in, and a
/// caller told "no snapshot" over a snapshot that IS there would be told a lie.
const TIME_STAMP_IS_MISSING: &str = "the last-backup-time marker did not reach the disk";

/// A namespace whose backup did not go through, and what exactly did not happen.
pub struct FailedNamespaceBackup {
    pub namespace: String,
    pub reason: &'static str,
}

/// Snapshots every namespace into a backup folder of its own. Returns the
/// namespaces whose backup did NOT go through — empty on a clean run.
///
/// Every failure below is reported and swallowed, never unwrapped: the whole
/// loop runs inside a single timer tick, so one namespace panicking took down
/// the backup of every namespace after it in the list. The failures are returned
/// rather than only logged so that a caller which can answer a human —
/// `MakeBackup` — does not report success over a namespace that has no snapshot.
pub async fn save_backup(app: &AppContext, force_write: bool) -> Vec<FailedNamespaceBackup> {
    let mut failed = Vec::new();

    for db_namespace in app.namespaces.get_all() {
        if let Some(reason) = save_namespace_backup(app, &db_namespace, force_write).await {
            failed.push(FailedNamespaceBackup {
                namespace: db_namespace.name.to_string(),
                reason,
            });
        }
    }

    failed
}

/// Snapshots ONE namespace, ignoring the interval — what `MakeBackup` asks for
/// when the request names a namespace. Same shape as `save_backup` above, so a
/// caller which reports failures does not have to care which of the two it
/// called.
pub async fn save_backup_of_namespace(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
) -> Vec<FailedNamespaceBackup> {
    match save_namespace_backup(app, db_namespace, true).await {
        Some(reason) => vec![FailedNamespaceBackup {
            namespace: db_namespace.name.to_string(),
            reason,
        }],
        None => Vec::new(),
    }
}

/// `None` means this namespace is left with its snapshots in the state they
/// should be in — which includes the namespaces there was nothing to do for.
/// Anything else is the reason it is not.
async fn save_namespace_backup(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    force_write: bool,
) -> Option<&'static str> {
    let now = DateTimeAsMicroseconds::now();

    if !force_write {
        if let Some(last_backup_time) = get_last_backup_time(app, db_namespace).await {
            let backup_interval_seconds = app.settings.backup_interval_hours * 60 * 60;

            if now
                .duration_since(last_backup_time)
                .as_positive_or_zero()
                .as_secs()
                < backup_interval_seconds
            {
                return None;
            }
        }
    }

    // A namespace with no tables produces an empty archive — 22 bytes, just the
    // zip end-of-central-directory record. Written every interval it is
    // indistinguishable from a real snapshot to the collector, so it occupies a
    // MaxBackupsToKeep slot forever and pushes a genuine backup out. Nothing to
    // back up means no file at all.
    //
    // The timestamp is still moved forward, so an empty namespace is
    // re-checked once per interval rather than on every single tick.
    if db_namespace.tables_amount() == 0 {
        my_logger::LOGGER.write_info(
            "Backup",
            "Namespace holds no tables - skipping the backup",
            LogEventCtx::new().add("namespace", db_namespace.name.to_string()),
        );

        return save_last_backup_time(app, db_namespace, now).await;
    }

    let file_name = now.to_rfc3339().replace(":", "").replace("-", "");

    let file_name = compile_backup_file(
        app,
        &db_namespace.name,
        format!("{}.zip", &file_name[..15]).as_str(),
    );

    // Straight into the file: the archive of a namespace weighs what its tables
    // weigh, and holding it in memory on top of them made the backup tick the
    // moment the process was most likely to be killed for its size.
    if let Err(err) = super::super::write_db_snapshot_as_zip_file(db_namespace, file_name).await {
        // Reported rather than unwrapped, as everything else here: a panic takes
        // the whole tick down, and with it the backup of every namespace after
        // this one, which is the failure this module exists to not have.
        my_logger::LOGGER.write_error(
            "Backup",
            format!("Can not write the snapshot. Err: {}", err),
            LogEventCtx::new().add("namespace", db_namespace.name.to_string()),
        );

        // The time stamp is deliberately left where it was: the next tick
        // retries instead of counting a snapshot which never reached the disk
        // as done and waiting out a whole interval.
        return Some(SNAPSHOT_IS_MISSING);
    }

    let time_stamp_failure = save_last_backup_time(app, db_namespace, now).await;

    super::gc_namespace_backups(app, db_namespace).await;

    time_stamp_failure
}

/// Writes one file of a namespace's backup folder, creating the folder when it
/// is missing. Returns whether the content reached the disk.
///
/// The folder can be missing under a namespace that exists: `db/<ns>` is
/// created when the namespace is opened, `backup/<ns>` only when the namespace
/// is backed up — or skipped — for the first time. A namespace which has never
/// been through a backup tick has the first but not the second.
async fn write_backup_file(file_name: &str, content: Vec<u8>) -> bool {
    if let Some(folder) = std::path::Path::new(file_name).parent() {
        if let Err(err) = tokio::fs::create_dir_all(folder).await {
            my_logger::LOGGER.write_error(
                "Backup",
                format!("Can not create backup folder. Err: {}", err),
                LogEventCtx::new().add("folder", format!("{}", folder.display())),
            );
            return false;
        }
    }

    if let Err(err) = tokio::fs::write(file_name, content).await {
        my_logger::LOGGER.write_error(
            "Backup",
            format!("Can not write backup file. Err: {}", err),
            LogEventCtx::new().add("fileName", file_name.to_string()),
        );
        return false;
    }

    true
}

/// When this namespace was backed up last, as far as the disk knows.
///
/// The `.last_backup_time` marker answers whenever it is readable. When it is
/// not, the newest snapshot in the folder answers instead: without that fallback
/// a namespace whose marker did not reach the disk looks exactly like one that
/// was never backed up, so every single tick writes another full snapshot while
/// the collector evicts the older ones — a whole history of backups collapses
/// into the last few minutes, with one error line per minute as the only hint.
async fn get_last_backup_time(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
) -> Option<DateTimeAsMicroseconds> {
    if let Some(from_the_marker) = read_last_backup_time(app, db_namespace).await {
        return Some(from_the_marker);
    }

    newest_snapshot_time(&super::get_list_of_files(app, db_namespace).await)
}

/// Moment of the newest snapshot of the folder, or `None` when it holds none.
///
/// Re-derives "newest" from the time stamps instead of trusting the position of
/// a file in the vector it is handed — the same reason
/// `select_backups_to_delete` does.
fn newest_snapshot_time(files: &[SnapshotFileModel]) -> Option<DateTimeAsMicroseconds> {
    let newest = files.iter().map(|itm| itm.modified_unix_seconds).max()?;

    Some(DateTimeAsMicroseconds::new(newest * 1_000_000))
}

/// The `.last_backup_time` marker, when there is a readable one.
///
/// A namespace which was never backed up simply has no marker — that is the
/// normal first tick, not a failure, and it is deliberately not reported.
/// Anything else is: an unreadable or unparsable marker is what sends the
/// interval calculation back to the fallback above.
async fn read_last_backup_time(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
) -> Option<DateTimeAsMicroseconds> {
    let file_name = compile_backup_file(app, &db_namespace.name, LAST_TIME_BACKUP_FILE_NAME);

    let content = match tokio::fs::read(file_name.as_str()).await {
        Ok(content) => content,
        Err(err) => {
            if err.kind() != std::io::ErrorKind::NotFound {
                my_logger::LOGGER.write_error(
                    "Backup",
                    format!("Can not read the last backup time. Err: {}", err),
                    LogEventCtx::new().add("fileName", file_name),
                );
            }

            return None;
        }
    };

    let content = match String::from_utf8(content) {
        Ok(content) => content,
        Err(err) => {
            my_logger::LOGGER.write_error(
                "Backup",
                format!("Can not read the last backup time. Err: {}", err),
                LogEventCtx::new().add("fileName", file_name),
            );

            return None;
        }
    };

    let result = DateTimeAsMicroseconds::from_str(content.as_str());

    if result.is_none() {
        my_logger::LOGGER.write_error(
            "Backup",
            "Can not parse the last backup time",
            LogEventCtx::new()
                .add("fileName", file_name)
                .add("content", content),
        );
    }

    result
}

/// Records the moment this namespace was backed up. `None` when the marker is on
/// the disk, otherwise the reason it is not.
///
/// The result is not ignored: the marker is what the next tick reads to decide
/// whether the interval has elapsed, so a namespace whose snapshot is written
/// and whose marker is not is in a state nobody can see from the outside — and
/// one the fallback in `get_last_backup_time` merely papers over.
async fn save_last_backup_time(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    now: DateTimeAsMicroseconds,
) -> Option<&'static str> {
    let file_name = compile_backup_file(app, &db_namespace.name, LAST_TIME_BACKUP_FILE_NAME);

    if write_backup_file(file_name.as_str(), now.to_rfc3339().into_bytes()).await {
        return None;
    }

    Some(TIME_STAMP_IS_MISSING)
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

    #[tokio::test]
    async fn test_missing_backup_folder_is_created_not_panicked_on() {
        // The production case: a namespace exists, its backup folder does not.
        // This used to be an unwrap() over ENOENT and it panicked the tick.
        let dir = std::env::temp_dir().join(format!("backup_write_{}", uuid::Uuid::new_v4()));
        let file_name = format!("{}/never-created-ns/.last_backup_time", dir.display());

        assert!(super::write_backup_file(file_name.as_str(), b"content".to_vec()).await);
        assert_eq!(
            b"content".to_vec(),
            tokio::fs::read(file_name.as_str()).await.unwrap()
        );

        tokio::fs::remove_dir_all(dir).await.ok();
    }

    #[tokio::test]
    async fn test_unwritable_path_is_reported_not_panicked_on() {
        // A file where a folder is expected: create_dir_all can not resolve it,
        // so the write is refused — and the caller keeps its time stamp.
        let dir = std::env::temp_dir().join(format!("backup_write_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();

        let occupied = format!("{}/ns", dir.display());
        tokio::fs::write(occupied.as_str(), b"i am a file")
            .await
            .unwrap();

        assert_eq!(
            false,
            super::write_backup_file(format!("{}/backup.zip", occupied).as_str(), b"x".to_vec())
                .await
        );

        tokio::fs::remove_dir_all(dir).await.ok();
    }

    #[test]
    fn test_the_newest_snapshot_answers_when_the_marker_is_gone() {
        // Whatever order the folder is listed in, the fallback is the LATEST
        // snapshot: anything older would let the interval look elapsed and
        // trigger a full snapshot on the very next tick.
        let files = vec![
            file("20260731T120000.zip", 200),
            file("20260731T140000.zip", 400),
            file("20260731T130000.zip", 300),
        ];

        let result = newest_snapshot_time(&files).unwrap();

        assert_eq!(400 * 1_000_000, result.unix_microseconds);
    }

    #[test]
    fn test_no_snapshots_means_no_fallback() {
        // A namespace which was never backed up: nothing to derive a time from,
        // so the backup has to happen now.
        assert!(newest_snapshot_time(&[]).is_none());
    }
}
