use core::str;
use std::{
    collections::BTreeMap,
    io::{Read, Seek},
    sync::Arc,
    time::Duration,
};

use my_no_sql_sdk::core::db::DbRow;
use my_no_sql_sdk::core::db_json_entity::DbJsonEntity;
use my_no_sql_sdk::server::rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{
    app::{AppContext, DbNamespace},
    db_operations::DbOperationError,
    db_sync::{states::InitTableEventSyncData, EventSource, SyncEvent},
    scripts::serializers::table_attrs::TableMetadataFileContract,
    zip::ZipReader,
};

use super::RestoreFileName;

#[derive(Debug)]
pub enum BackupError {
    TableNotFoundInBackupFile,
    #[allow(dead_code)]
    InvalidFileName(String),
    #[allow(dead_code)]
    FileReadError(String),
    #[allow(dead_code)]
    ZipArchiveError(String),
    #[allow(dead_code)]
    TableNotFoundToRestoreBackupAndNoMetadataFound(String),
    #[allow(dead_code)]
    InvalidFileContent {
        file_name: String,
        partition_key: String,
        err: String,
    },
    /// The database refused a write of the restore — the server is not
    /// initialized yet, the table is gone, and so on.
    DbOperation(DbOperationError),
}

impl BackupError {
    pub fn into_message(self) -> String {
        match self {
            BackupError::TableNotFoundInBackupFile => {
                "Table not found in the backup file".to_string()
            }
            BackupError::InvalidFileName(err) => format!("Invalid file name: {}", err),
            BackupError::FileReadError(err) => err,
            BackupError::ZipArchiveError(err) => format!("Zip archive error: {}", err),
            BackupError::TableNotFoundToRestoreBackupAndNoMetadataFound(table) => format!(
                "Table '{}' is not present on the server and the snapshot has no metadata to recreate it",
                table
            ),
            BackupError::InvalidFileContent {
                file_name,
                partition_key,
                err,
            } => format!(
                "Invalid content in '{}' (partition '{}'): {}",
                file_name, partition_key, err
            ),
            BackupError::DbOperation(err) => format!("{:?}", err),
        }
    }
}

/// Reads a snapshot file by name from the server's backup folder and restores
/// it. `table_name == None` restores every table found in the snapshot.
pub async fn restore_from_file(
    app: &Arc<AppContext>,
    db_namespace: &Arc<DbNamespace>,
    file_name: &str,
    table_name: Option<&str>,
    clean_table: bool,
) -> Result<(), BackupError> {
    if !super::utils::backup_file_name_is_valid(file_name) {
        return Err(BackupError::InvalidFileName(file_name.to_string()));
    }

    let full_path = super::utils::compile_backup_file(app, &db_namespace.name, file_name);

    // Opened, not read: a snapshot weighs what the data it holds weighs, and
    // holding it as well as what is being restored out of it is what made
    // restoring, not the traffic, the peak of the process.
    let zip_reader = ZipReader::open_file_on_blocking_thread(full_path)
        .await
        .map_err(|err| {
            BackupError::FileReadError(format!("Error loading file '{}': {}", file_name, err))
        })?;

    restore_from_zip_reader(app, db_namespace, zip_reader, table_name, clean_table).await
}

/// Restores an archive which is already in memory — the one uploaded to
/// `RestoreFromZip`. A snapshot in the backup folder goes through
/// `restore_from_file`, which does not read it whole.
pub async fn restore(
    app: &Arc<AppContext>,
    db_namespace: &Arc<DbNamespace>,
    backup_content: Vec<u8>,
    table_name: Option<&str>,
    clean_table: bool,
) -> Result<(), BackupError> {
    let zip_reader = ZipReader::in_memory(backup_content)
        .map_err(|err| BackupError::ZipArchiveError(err.to_string()))?;

    restore_from_zip_reader(app, db_namespace, zip_reader, table_name, clean_table).await
}

async fn restore_from_zip_reader<TReader: Read + Seek + Send + 'static>(
    app: &Arc<AppContext>,
    db_namespace: &Arc<DbNamespace>,
    mut zip_reader: ZipReader<TReader>,
    table_name: Option<&str>,
    clean_table: bool,
) -> Result<(), BackupError> {
    let mut partitions: BTreeMap<String, Vec<RestoreFileName>> = BTreeMap::new();

    for file_name_str in zip_reader.get_file_names() {
        let file_name =
            RestoreFileName::new(file_name_str).map_err(|err| BackupError::InvalidFileName(err))?;

        let Some(file_name) = file_name else {
            println!("Skipping  file [{}]", file_name_str);
            continue;
        };

        match partitions.get_mut(&file_name.table_name) {
            Some(by_table) => {
                if file_name.file_type.is_metadata() {
                    by_table.insert(0, file_name);
                } else {
                    by_table.push(file_name)
                }
            }
            None => {
                partitions.insert(file_name.table_name.to_string(), vec![file_name]);
            }
        }
    }

    if partitions.is_empty() {
        return Err(BackupError::TableNotFoundInBackupFile);
    }

    match table_name {
        Some(table_name) => match partitions.remove(table_name) {
            Some(files) => {
                restore_to_db(
                    &app,
                    db_namespace,
                    table_name,
                    files,
                    zip_reader,
                    clean_table,
                )
                .await?;
            }
            None => {
                return Err(BackupError::TableNotFoundInBackupFile);
            }
        },
        None => {
            for (table_name, files) in partitions {
                // The reader is passed along and handed back: reading an entry
                // happens on a blocking thread, which it has to be moved into.
                zip_reader = restore_to_db(
                    &app,
                    db_namespace,
                    table_name.as_str(),
                    files,
                    zip_reader,
                    clean_table,
                )
                .await?;
            }
        }
    }
    Ok(())
}

/// Restores a single partition of a table from a snapshot file. The table must
/// already exist on the server (restore the whole table first if it does not).
/// The partition content is replaced with the rows stored in the snapshot.
pub async fn restore_partition(
    app: &Arc<AppContext>,
    db_namespace: &Arc<DbNamespace>,
    file_name: &str,
    table_name: &str,
    partition_key: &str,
) -> Result<(), String> {
    let content = super::read_snapshot_partition_rows(
        app,
        db_namespace,
        file_name,
        table_name,
        partition_key,
    )
    .await
    .map_err(|err| err.into_message())?;

    let db_rows = parse_partition_rows(content)
        .await
        .map_err(|err| format!("Invalid partition content: {}", err))?;

    let db_table = db_namespace.db.get_table(table_name).ok_or_else(|| {
        format!(
            "Table '{}' does not exist on the server. Restore the whole table first.",
            table_name
        )
    })?;

    let persist_moment = DateTimeAsMicroseconds::now().add(Duration::from_secs(5));

    crate::db_operations::write::clean_partition_and_bulk_insert(
        app,
        db_namespace,
        &db_table,
        partition_key.to_string(),
        vec![(partition_key.to_string(), db_rows)],
        EventSource::Backup,
        persist_moment,
        DateTimeAsMicroseconds::now(),
    )
    .await
    .map_err(|err| format!("{:?}", err))?;

    // Emit a full table-init so subscribers re-initialize after the restore,
    // same as the whole-table restore path (see `restore_to_db`).
    {
        let table_data = db_table.data.read();
        let sync_data = InitTableEventSyncData::new(&table_data, EventSource::Backup);
        crate::operations::sync::dispatch(app, db_namespace, SyncEvent::InitTable(sync_data));
    }

    Ok(())
}

/// Parses the rows of a partition as they are stored in an archive.
///
/// CPU and allocation on the size of the partition, and a row of it is copied
/// into a buffer of its own, so it happens off the runtime worker.
async fn parse_partition_rows(content: Vec<u8>) -> Result<Vec<Arc<DbRow>>, String> {
    tokio::task::spawn_blocking(move || {
        DbJsonEntity::restore_as_vec(content.as_slice()).map_err(|err| format!("{:?}", err))
    })
    .await
    .map_err(|err| format!("The parse task did not finish. Err: {}", err))?
}

/// Restores every file of one table out of the archive, handing the reader back
/// for the next table.
async fn restore_to_db<TReader: Read + Seek + Send + 'static>(
    app: &Arc<AppContext>,
    db_namespace: &Arc<DbNamespace>,
    table_name: &str,
    mut files: Vec<RestoreFileName>,
    mut zip: ZipReader<TReader>,
    clean_table: bool,
) -> Result<ZipReader<TReader>, BackupError> {
    let persist_moment = DateTimeAsMicroseconds::now().add(Duration::from_secs(5));
    let starts_with_metadata = files
        .first()
        .map(|file| file.file_type.is_metadata())
        .unwrap_or(false);

    let db_table = if starts_with_metadata {
        let metadata_file = files.remove(0);

        let content;
        (zip, content) = zip
            .read_entry(metadata_file.file_name)
            .await
            .map_err(|err| BackupError::ZipArchiveError(err.to_string()))?;

        let table = TableMetadataFileContract::parse(content.as_slice());

        crate::db_operations::write::table::create_if_not_exist(
            app,
            db_namespace,
            table_name,
            table.persist,
            table.max_partitions_amount,
            table.max_rows_per_partition_amount,
            EventSource::Backup,
            persist_moment,
        )
        .await
        .map_err(BackupError::DbOperation)?
    } else {
        let Some(db_table) = db_namespace.db.get_table(table_name) else {
            return Err(BackupError::TableNotFoundToRestoreBackupAndNoMetadataFound(
                table_name.to_string(),
            ));
        };

        db_table
    };

    if clean_table {
        crate::db_operations::write::clean_table(
            &app,
            db_namespace,
            &db_table,
            EventSource::Backup,
            persist_moment,
        )
        .await
        .map_err(BackupError::DbOperation)?;
    }

    for partition_file in files {
        let partition_key = partition_file.file_type.unwrap_as_partition_key();
        let file_name = partition_file.file_name;

        let content;
        (zip, content) = zip
            .read_entry(file_name.clone())
            .await
            .map_err(|err| BackupError::ZipArchiveError(err.to_string()))?;

        let db_rows =
            parse_partition_rows(content)
                .await
                .map_err(|err| BackupError::InvalidFileContent {
                    file_name,
                    partition_key: partition_key.to_string(),
                    err,
                })?;

        crate::db_operations::write::clean_partition_and_bulk_insert(
            app,
            db_namespace,
            &db_table,
            partition_key.to_string(),
            vec![(partition_key, db_rows)],
            EventSource::Backup,
            persist_moment,
            DateTimeAsMicroseconds::now(),
        )
        .await
        .map_err(BackupError::DbOperation)?;
    }

    // The per-partition writes above only emit InitPartitions events, so a
    // reader subscribed to the whole table never receives a fresh full
    // snapshot after a restore. Dispatch a table-init with the now-restored
    // state so every subscriber re-initializes its local copy of the table.
    {
        let table_data = db_table.data.read();
        let sync_data = InitTableEventSyncData::new(&table_data, EventSource::Backup);
        crate::operations::sync::dispatch(app, db_namespace, SyncEvent::InitTable(sync_data));
    }

    Ok(zip)
}
