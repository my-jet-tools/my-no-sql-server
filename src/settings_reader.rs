use my_no_sql_sdk::core::rust_extensions;

use serde::{Deserialize, Serialize};

use crate::files_repo::FilesRepo;
use crate::persist_repo::PersistRepo;

#[derive(Serialize, Deserialize, Debug)]
pub struct SettingsModel {
    #[serde(rename = "PersistenceDest")]
    pub persistence_dest: String,

    #[serde(rename = "Location")]
    pub location: String,

    #[serde(rename = "CompressData")]
    pub compress_data: bool,

    #[serde(rename = "TableApiKey")]
    pub table_api_key: String,

    #[serde(rename = "SkipBrokenPartitions")]
    pub skip_broken_partitions: bool,

    #[serde(rename = "InitThreadsAmount")]
    pub init_threads_amount: usize,

    #[serde(rename = "TcpSendTimeoutSec")]
    pub tcp_send_time_out: u64,

    #[serde(rename = "BackupFolder")]
    backup_folder: String,

    #[serde(rename = "BackupIntervalHours")]
    pub backup_interval_hours: u64,

    #[serde(rename = "MaxBackupsToKeep")]
    pub max_backups_to_keep: usize,

    #[serde(rename = "AutoCreateTableOnReaderSubscribe")]
    pub auto_create_table_on_reader_subscribe: bool,

    #[serde(rename = "InitFromOtherServerUrl")]
    pub init_from_other_server_url: Option<String>,
}

impl SettingsModel {
    /// `PersistenceDest` with the `~` and the environment variables resolved.
    pub fn get_persistence_dest(&self) -> String {
        my_no_sql_sdk::server::rust_extensions::file_utils::format_path(
            self.persistence_dest.as_str(),
        )
        .to_string()
    }

    /// Opens the persistence backend of a single namespace. `PersistenceDest` is
    /// a directory, and every namespace is a folder of its own inside it — the
    /// default one included.
    pub async fn open_persist_repo(&self, namespace: &str) -> PersistRepo {
        let dest = self.get_persistence_dest();

        let folder = crate::persist_repo::get_namespace_folder(dest.as_str(), namespace);

        PersistRepo::new(FilesRepo::open(folder, self.skip_broken_partitions).await)
    }

    pub fn get_backup_folder<'s>(&'s self) -> rust_extensions::StrOrString<'s> {
        rust_extensions::file_utils::format_path(self.backup_folder.as_str())
    }

    pub fn get_init_from_other_server_url(&self) -> Option<&str> {
        if let Some(url) = &self.init_from_other_server_url {
            return Some(url.as_str());
        }

        None
    }
}

pub async fn read_settings() -> SettingsModel {
    let file_name = rust_extensions::file_utils::format_path("~/.mynosqlserver");

    let file_content = tokio::fs::read(file_name.as_str()).await;

    if let Err(err) = &file_content {
        panic!(
            "Can't open settings file [{}]. Err: {}",
            file_name.as_str(),
            err
        );
    }

    let file_content = file_content.unwrap();

    let result: SettingsModel = serde_yaml::from_slice(file_content.as_slice()).unwrap();

    result
}

/*
fn get_settings_filename() -> String {
    let path = env!("HOME");

    if path.ends_with('/') {
        return format!("{}{}", path, ".mynosqlserver");
    }

    return format!("{}{}", path, "/.mynosqlserver");
}
 */
