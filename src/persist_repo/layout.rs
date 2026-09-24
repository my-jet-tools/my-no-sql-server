use my_no_sql_sdk::core::db::DbNamespaceName;
use my_no_sql_sdk::{validate_namespace_name, DEFAULT_NAMESPACE};

/// Every namespace persists into a folder of its own inside the persistence root,
/// the default namespace included:
///
/// ```text
/// <root>/default/512, /1024, /tables.meta
/// <root>/alpha/512,   /1024, /tables.meta
/// ```
pub fn get_namespace_folder(root: &str, namespace: &str) -> String {
    format!("{}/{}", root, namespace)
}

/// Moves a pre-namespace persistence root into `<root>/default`.
///
/// Before namespaces existed everything lived directly in the root: the
/// page-files (`512`, `1024`, …) and `tables.meta`. Namespaces turned the root
/// into a directory of directories, so those files have to move down one level
/// once.
///
/// The trigger is "the root still holds files", not "there is no `default`
/// folder": that makes the migration idempotent and crash-safe. A process which
/// dies half way through leaves some files in the root and some in `default/`,
/// and the next start simply carries the rest down.
pub async fn migrate_legacy_root_into_default_namespace(root: &str) {
    let mut read_dir = match tokio::fs::read_dir(root).await {
        Ok(read_dir) => read_dir,
        // No root yet — a first ever start. Nothing to migrate; the namespace
        // folder is created when its persistence backend is opened.
        Err(_) => return,
    };

    let mut files_to_move = Vec::new();

    loop {
        let entry = match read_dir.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(err) => panic!(
                "Can not list the persistence root {} while checking it for the namespaces migration. Err: {}",
                root, err
            ),
        };

        let file_type = match entry.file_type().await {
            Ok(file_type) => file_type,
            Err(err) => panic!(
                "Can not read the type of {:?} while checking the persistence root for the namespaces migration. Err: {}",
                entry.path(),
                err
            ),
        };

        // Directories in the root are namespaces (or an already migrated
        // `default`) — only loose files belong to the pre-namespace layout.
        if file_type.is_file() {
            files_to_move.push(entry.file_name());
        }
    }

    if files_to_move.is_empty() {
        return;
    }

    let default_folder = get_namespace_folder(root, DEFAULT_NAMESPACE);

    println!(
        "Persistence root {} holds {} file(s) of the pre-namespace layout. Moving them into {}",
        root,
        files_to_move.len(),
        default_folder
    );

    tokio::fs::create_dir_all(default_folder.as_str())
        .await
        .unwrap_or_else(|err| {
            panic!(
                "Can not create the default namespace folder {}. Err: {}",
                default_folder, err
            )
        });

    for file_name in files_to_move {
        let src = format!("{}/{}", root, file_name.to_string_lossy());
        let dest = format!("{}/{}", default_folder, file_name.to_string_lossy());

        tokio::fs::rename(src.as_str(), dest.as_str())
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "Can not move {} into the default namespace folder as {}. Err: {}",
                    src, dest, err
                )
            });

        println!("Moved {} -> {}", src, dest);
    }
}

/// Namespaces which already have a folder on disk. Called at start up, after the
/// legacy migration, to load every namespace this server persisted before.
pub async fn get_namespaces_on_disk(root: &str) -> Vec<DbNamespaceName> {
    let mut result = Vec::new();

    let mut read_dir = match tokio::fs::read_dir(root).await {
        Ok(read_dir) => read_dir,
        Err(_) => return result,
    };

    loop {
        let entry = match read_dir.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(err) => panic!(
                "Can not list the persistence root {} while looking for the namespaces. Err: {}",
                root, err
            ),
        };

        let is_dir = match entry.file_type().await {
            Ok(file_type) => file_type.is_dir(),
            Err(_) => false,
        };

        if !is_dir {
            continue;
        }

        let folder_name = entry.file_name().to_string_lossy().to_string();

        // A folder we can not read as a namespace name is somebody else's — say
        // so and leave it alone rather than guessing.
        if let Err(err) = validate_namespace_name(folder_name.as_str()) {
            println!(
                "WARNING: Folder {} inside the persistence root {} is not a valid namespace name and is skipped. {}",
                folder_name, root, err
            );
            continue;
        }

        result.push(folder_name.into());
    }

    result
}
