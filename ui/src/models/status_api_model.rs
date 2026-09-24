use serde::*;

use super::{ReaderApiModel, StatusBarApiModel, TableApiModel, WriterApiModel};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatusApiModel {
    #[serde(rename = "notInitialized", skip_serializing_if = "Option::is_none")]
    pub not_initialized: Option<NotInitializedApiModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initialized: Option<InitializedApiModel>,
    #[serde(rename = "statusBar")]
    pub status_bar: StatusBarApiModel,
}

impl Default for StatusApiModel {
    fn default() -> Self {
        Self {
            not_initialized: None,
            initialized: None,
            status_bar: StatusBarApiModel::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InitializedApiModel {
    pub readers: Vec<ReaderApiModel>,
    pub writers: Vec<WriterApiModel>,
    pub tables: Vec<TableApiModel>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NotInitializedApiModel {
    #[serde(default)]
    pub message: Option<String>,
    #[serde(rename = "loadedFiles", default)]
    pub loaded_files: Option<u64>,
    #[serde(rename = "filesToLoad", default)]
    pub files_to_load: Option<u64>,
}
