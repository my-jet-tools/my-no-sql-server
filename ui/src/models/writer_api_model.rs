use serde::*;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct WriterApiModel {
    pub name: String,
    #[serde(default = "crate::models::default_namespace")]
    pub namespace: String,
    pub version: String,
    pub last_update: String,
    #[serde(default)]
    pub tables: Vec<String>,
}
