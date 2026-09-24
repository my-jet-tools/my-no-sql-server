use my_no_sql_sdk::core::db::DbNamespaceName;
use my_no_sql_sdk::DEFAULT_NAMESPACE;

use crate::app::AppContext;

pub const LAST_TIME_BACKUP_FILE_NAME: &str = ".last_backup_time";

/// Backups are laid out the same way the persistence root is: a folder per
/// namespace, the default one included. Each folder keeps its own snapshots and
/// its own `.last_backup_time`, so `MaxBackupsToKeep` is counted per namespace.
pub fn get_backup_folder(app: &AppContext, namespace: &DbNamespaceName) -> String {
    let backup_folder = app.settings.get_backup_folder();

    format!(
        "{}/{}",
        backup_folder.as_str().trim_end_matches(['/', '\\']),
        namespace
    )
}

pub fn compile_backup_file(
    app: &AppContext,
    namespace: &DbNamespaceName,
    file_name: &str,
) -> String {
    format!("{}/{}", get_backup_folder(app, namespace), file_name)
}

/// A snapshot is addressed by its bare file name inside the namespace's own
/// backup folder. Anything with a path separator in it would step out of that
/// folder — into another namespace's backups, or out of the backup folder
/// altogether — so it is refused rather than resolved.
pub fn backup_file_name_is_valid(file_name: &str) -> bool {
    !(file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains("..")
        || !file_name.ends_with(".zip"))
}

/// Moves a pre-namespace backup folder into `<backup_folder>/default`.
///
/// Same shape as the persistence-root migration: the trigger is "the backup
/// folder still holds files", which makes it idempotent and lets a run
/// interrupted half way finish on the next start.
pub async fn migrate_legacy_backup_folder(app: &AppContext) {
    let backup_folder = app.settings.get_backup_folder().to_string();
    let backup_folder = backup_folder.trim_end_matches(['/', '\\']);

    let mut read_dir = match tokio::fs::read_dir(backup_folder).await {
        Ok(read_dir) => read_dir,
        // No backup folder yet — nothing to migrate.
        Err(_) => return,
    };

    let mut files_to_move = Vec::new();

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let is_file = match entry.file_type().await {
            Ok(file_type) => file_type.is_file(),
            Err(_) => false,
        };

        if is_file {
            files_to_move.push(entry.file_name().to_string_lossy().to_string());
        }
    }

    if files_to_move.is_empty() {
        return;
    }

    let default_folder = format!("{}/{}", backup_folder, DEFAULT_NAMESPACE);

    println!(
        "Backup folder {} holds {} file(s) of the pre-namespace layout. Moving them into {}",
        backup_folder,
        files_to_move.len(),
        default_folder
    );

    if let Err(err) = tokio::fs::create_dir_all(default_folder.as_str()).await {
        println!(
            "Can not create the default namespace backup folder {}. Err: {}",
            default_folder, err
        );
        return;
    }

    for file_name in files_to_move {
        let src = format!("{}/{}", backup_folder, file_name);
        let dest = format!("{}/{}", default_folder, file_name);

        match tokio::fs::rename(src.as_str(), dest.as_str()).await {
            Ok(_) => println!("Moved {} -> {}", src, dest),
            // A backup which can not be moved is not worth refusing to start
            // over: it stays where it is and is simply not listed any more.
            Err(err) => println!("Can not move {} into {}. Err: {}", src, dest, err),
        }
    }
}
