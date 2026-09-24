use my_no_sql_sdk::core::db::DbTableAttributes;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use serde::{Deserialize, Serialize};

// `Option` fields are `#[serde(skip_serializing_if = "Option::is_none")]` so a
// `None` is omitted entirely instead of written as `null`, keeping `tables.meta`
// small. Each also carries `#[serde(default)]` so an omitted field deserializes
// back to `None` (the write side no longer emits it).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TableMetadataFileContract {
    #[serde(rename = "Persist")]
    #[serde(default = "default_persist")]
    pub persist: bool,
    #[serde(rename = "MaxPartitionsAmount")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_partitions_amount: Option<usize>,
    #[serde(rename = "MaxRowsPerPartitionAmount")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_rows_per_partition_amount: Option<usize>,
    // Backwards-compatible: missing in metadata written before the feature -> None -> not compressed.
    #[serde(rename = "Compressed")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compressed: Option<bool>,
    #[serde(rename = "Created")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
}

impl TableMetadataFileContract {
    pub fn parse(content: &[u8]) -> Self {
        let parse_result = serde_json::from_slice::<TableMetadataFileContract>(content);

        match parse_result {
            Ok(res) => res,
            Err(_) => TableMetadataFileContract {
                max_partitions_amount: None,
                max_rows_per_partition_amount: None,
                persist: true,
                compressed: None,
                created: Some(DateTimeAsMicroseconds::now().to_rfc3339()),
            },
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

fn default_persist() -> bool {
    true
}

impl Into<TableMetadataFileContract> for &'_ DbTableAttributes {
    fn into(self) -> TableMetadataFileContract {
        TableMetadataFileContract {
            persist: self.persist,
            max_partitions_amount: self.max_partitions_amount,
            max_rows_per_partition_amount: self.max_rows_per_partition_amount,
            compressed: Some(self.compressed),
            created: self.created.to_rfc3339().into(),
        }
    }
}

impl Into<DbTableAttributes> for TableMetadataFileContract {
    fn into(self) -> DbTableAttributes {
        let mut result = DbTableAttributes {
            created: if let Some(created) = &self.created {
                match DateTimeAsMicroseconds::from_str(created) {
                    Some(value) => value,
                    None => DateTimeAsMicroseconds::now(),
                }
            } else {
                DateTimeAsMicroseconds::now()
            },
            max_partitions_amount: self.max_partitions_amount,
            max_rows_per_partition_amount: self.max_rows_per_partition_amount,
            persist: self.persist,
            compressed: self.compressed.unwrap_or(false),
        };

        if let Some(value) = result.max_partitions_amount {
            if value == 0 {
                result.max_partitions_amount = None;
            }
        }

        if let Some(value) = result.max_rows_per_partition_amount {
            if value == 0 {
                result.max_rows_per_partition_amount = None;
            }
        }

        result
    }
}

pub fn serialize(attrs: &DbTableAttributes) -> Vec<u8> {
    let contract = TableMetadataFileContract {
        max_partitions_amount: attrs.max_partitions_amount,
        max_rows_per_partition_amount: attrs.max_rows_per_partition_amount,
        persist: attrs.persist,
        compressed: Some(attrs.compressed),
        created: Some(attrs.created.to_rfc3339()),
    };

    contract.to_vec()
}
