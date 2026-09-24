use std::path::PathBuf;

use my_no_sql_sdk::core::rust_extensions::file_utils;

use super::models::UiSettingsModel;

const FILE_NAME: &str = "ui-settings.json";

/// Returns the absolute path to `ui-settings.json` — sitting in the same
/// directory as the persistence root.
pub fn settings_path(persistence_dest: &str) -> PathBuf {
    let resolved = file_utils::format_path(persistence_dest);
    let path = PathBuf::from(resolved.as_str());
    let dir = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    dir.join(FILE_NAME)
}

pub async fn load(persistence_dest: &str) -> UiSettingsModel {
    let path = settings_path(persistence_dest);
    match tokio::fs::read(&path).await {
        Ok(bytes) => match serde_json::from_slice::<UiSettingsModel>(&bytes) {
            Ok(model) => model.sanitized(),
            Err(_) => UiSettingsModel::default(),
        },
        Err(_) => UiSettingsModel::default(),
    }
}

pub async fn save(persistence_dest: &str, model: &UiSettingsModel) -> std::io::Result<()> {
    let path = settings_path(persistence_dest);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let bytes = serde_json::to_vec_pretty(model)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    tokio::fs::write(&path, bytes).await
}
