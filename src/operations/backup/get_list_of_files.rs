use serde_derive::Serialize;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::{AppContext, DbNamespace};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SnapshotFileModel {
    pub name: String,
    pub size: i64,
    /// Modification time of the file, unix seconds. This — not the name — is
    /// what orders snapshots: the folder holds two generations of file names
    /// ("2026-06-27T07_09_54.zip" and "20260731T185201.zip") which are not
    /// lexicographically monotonic against each other, because '-' sorts before
    /// a digit. Sorting a mixed folder by name interleaves the generations and
    /// the GC then throws away the wrong end.
    #[serde(rename = "modified")]
    pub modified_unix_seconds: i64,
}

/// Snapshots of the namespace, NEWEST FIRST.
pub async fn get_list_of_files(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
) -> Vec<SnapshotFileModel> {
    let backup_folder = super::utils::get_backup_folder(app, &db_namespace.name);

    // A namespace which was never backed up simply has no folder yet.
    let mut read_dir = match tokio::fs::read_dir(backup_folder.as_str()).await {
        Ok(read_dir) => read_dir,
        Err(_) => return Vec::new(),
    };

    let mut result = Vec::new();

    while let Ok(entry) = read_dir.next_entry().await {
        if entry.is_none() {
            break;
        }

        let entry = entry.unwrap();

        let file_type = entry.file_type().await.unwrap();

        if !file_type.is_file() {
            continue;
        }

        let path = entry.path();

        let path = format!("{}", path.display());

        let file_name = extract_file_name(path.as_str(), std::path::MAIN_SEPARATOR);

        // `.last_backup_time` lives in the same folder and is deliberately not a
        // snapshot: it is neither listed nor eligible for collection.
        if !if_filename_is_backup(file_name) {
            continue;
        }

        let metadata = entry.metadata().await;

        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0) as i64;
        let modified_unix_seconds = metadata
            .ok()
            .and_then(|m| m.modified().ok())
            .map(to_unix_seconds)
            .unwrap_or(0);

        result.push(SnapshotFileModel {
            name: file_name.to_string(),
            size,
            modified_unix_seconds,
        });
    }

    sort_newest_first(&mut result);

    result
}

fn to_unix_seconds(modified: SystemTime) -> i64 {
    match modified.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs() as i64,
        // Older than the epoch — treat as ancient rather than as unknown.
        Err(_) => 0,
    }
}

/// Newest first, name as the tie-breaker so the order is stable when two
/// snapshots share a modification second.
pub fn sort_newest_first(files: &mut Vec<SnapshotFileModel>) {
    files.sort_by(|left, right| {
        right
            .modified_unix_seconds
            .cmp(&left.modified_unix_seconds)
            .then_with(|| right.name.cmp(&left.name))
    });
}

pub fn extract_file_name(full_path: &str, separator: char) -> &str {
    let full_path_as_bytes = full_path.as_bytes();

    for index in (0..full_path_as_bytes.len()).rev() {
        if full_path_as_bytes[index] == separator as u8 {
            return &full_path[index + 1..];
        }
    }

    panic!("Can not extract filename from full path [{}]", full_path);
}

fn if_filename_is_backup(src: &str) -> bool {
    return src.ends_with(".zip");
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
    fn test_sorted_newest_first() {
        let mut files = vec![
            file("20260731T140000.zip", 300),
            file("20260731T120000.zip", 100),
            file("20260731T130000.zip", 200),
        ];

        sort_newest_first(&mut files);

        let names: Vec<&str> = files.iter().map(|itm| itm.name.as_str()).collect();

        assert_eq!(
            vec![
                "20260731T140000.zip",
                "20260731T130000.zip",
                "20260731T120000.zip"
            ],
            names
        );
    }

    #[test]
    fn test_mixed_name_formats_are_ordered_by_time_not_by_name() {
        // The old name sorts BEFORE the new one lexicographically ('-' < '2'),
        // yet it is the newer file. Ordering must follow the timestamp.
        let mut files = vec![
            file("20260731T185201.zip", 100),
            file("2026-06-27T07_09_54.zip", 200),
        ];

        sort_newest_first(&mut files);

        assert_eq!("2026-06-27T07_09_54.zip", files[0].name.as_str());
        assert_eq!("20260731T185201.zip", files[1].name.as_str());
    }

    #[test]
    fn test_last_backup_time_marker_is_not_a_snapshot() {
        assert_eq!(false, if_filename_is_backup(".last_backup_time"));
        assert_eq!(true, if_filename_is_backup("20260731T185201.zip"));
    }

    #[test]
    fn test_a_snapshot_in_flight_is_not_a_snapshot() {
        // A snapshot is written under a temporary name and renamed into place, so
        // an archive being built must not be listed, downloaded, or picked by
        // MaxBackupsToKeep — all three go through this predicate.
        assert_eq!(false, if_filename_is_backup("20260731T185201.zip.tmp"));
    }
}
