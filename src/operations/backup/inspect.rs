use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;

use my_no_sql_sdk::server::rust_extensions::base64::FromBase64;
use serde_derive::Serialize;

use std::sync::Arc;

use crate::{
    app::{AppContext, DbNamespace},
    scripts::TABLE_METADATA_FILE_NAME,
    zip::ZipReader,
};

#[derive(Debug)]
pub enum InspectError {
    InvalidFileName,
    FileNotFound,
    IoError(String),
    TableNotFound,
    PartitionNotFound,
    InvalidPartitionKey,
}

impl InspectError {
    pub fn into_message(self) -> String {
        match self {
            InspectError::InvalidFileName => "Invalid snapshot file name".to_string(),
            InspectError::FileNotFound => "Snapshot file not found".to_string(),
            InspectError::IoError(s) => format!("I/O error: {}", s),
            InspectError::TableNotFound => "Table not found in snapshot".to_string(),
            InspectError::PartitionNotFound => "Partition not found in snapshot".to_string(),
            InspectError::InvalidPartitionKey => "Invalid partition key encoding".to_string(),
        }
    }
}

fn validate_file_name(file_name: &str) -> Result<(), InspectError> {
    if !super::utils::backup_file_name_is_valid(file_name) {
        return Err(InspectError::InvalidFileName);
    }
    Ok(())
}

/// Opens the snapshot without reading it: listing what is inside it costs its
/// central directory, and reading one partition out of it costs that partition.
async fn load_zip(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    file_name: &str,
) -> Result<ZipReader<BufReader<File>>, InspectError> {
    validate_file_name(file_name)?;
    let full_path = super::utils::compile_backup_file(app, &db_namespace.name, file_name);

    ZipReader::open_file_on_blocking_thread(full_path)
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => InspectError::FileNotFound,
            _ => InspectError::IoError(e.to_string()),
        })
}

#[derive(Serialize)]
pub struct SnapshotTable {
    pub name: String,
    #[serde(rename = "partitionsCount")]
    pub partitions_count: usize,
}

pub async fn list_snapshot_tables(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    file_name: &str,
) -> Result<Vec<SnapshotTable>, InspectError> {
    let mut zip = load_zip(app, db_namespace, file_name).await?;

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    for entry in zip.get_file_names() {
        let Some(idx) = entry.find('/') else { continue };
        let table = &entry[..idx];
        let rest = &entry[idx + 1..];
        if rest.is_empty() {
            continue;
        }
        let bump = if rest == TABLE_METADATA_FILE_NAME {
            0
        } else {
            1
        };
        *counts.entry(table.to_string()).or_insert(0) += bump;
    }

    Ok(counts
        .into_iter()
        .map(|(name, partitions_count)| SnapshotTable {
            name,
            partitions_count,
        })
        .collect())
}

pub async fn list_snapshot_partitions(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    file_name: &str,
    table_name: &str,
) -> Result<Vec<String>, InspectError> {
    let mut zip = load_zip(app, db_namespace, file_name).await?;

    let prefix = format!("{}/", table_name);
    let mut found_table = false;
    let mut partitions: Vec<String> = Vec::new();

    for entry in zip.get_file_names() {
        if !entry.starts_with(&prefix) {
            continue;
        }
        found_table = true;
        let rest = &entry[prefix.len()..];
        if rest.is_empty() || rest == TABLE_METADATA_FILE_NAME {
            continue;
        }
        let bytes = rest
            .from_base64()
            .map_err(|_| InspectError::InvalidPartitionKey)?;
        let pk = String::from_utf8(bytes).map_err(|_| InspectError::InvalidPartitionKey)?;
        partitions.push(pk);
    }

    if !found_table {
        return Err(InspectError::TableNotFound);
    }

    partitions.sort();
    Ok(partitions)
}

pub async fn read_snapshot_partition_rows(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    file_name: &str,
    table_name: &str,
    partition_key: &str,
) -> Result<Vec<u8>, InspectError> {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(partition_key.as_bytes());
    let zip_path = format!("{}/{}", table_name, encoded);

    let zip = load_zip(app, db_namespace, file_name).await?;

    let (_, content) = zip
        .read_entry(zip_path)
        .await
        .map_err(|_| InspectError::PartitionNotFound)?;

    Ok(content)
}
