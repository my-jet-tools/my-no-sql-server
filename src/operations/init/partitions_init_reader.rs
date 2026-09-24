use std::collections::HashMap;
use std::sync::Arc;

use my_no_sql_sdk::core::{db::DbRow, db_json_entity::DbJsonEntity};

use crate::persist_repo::LoadedPartition;

use super::EntitiesInitReader;

/// Init reader for the local persistence backend. Decompresses
/// each partition blob and parses it into rows. A broken partition is either
/// skipped (with a log) or fatal, per `SkipBrokenPartitions`.
pub struct PartitionsInitReader {
    by_table: HashMap<String, Vec<LoadedPartition>>,
    skip_errors: bool,
}

impl PartitionsInitReader {
    pub fn new(partitions: Vec<LoadedPartition>, skip_errors: bool) -> Self {
        let mut by_table: HashMap<String, Vec<LoadedPartition>> = HashMap::new();
        for partition in partitions {
            by_table
                .entry(partition.table_name.clone())
                .or_default()
                .push(partition);
        }

        Self {
            by_table,
            skip_errors,
        }
    }
}

#[async_trait::async_trait]
impl EntitiesInitReader for PartitionsInitReader {
    async fn get_entities(&mut self, table_name: &str) -> Option<Vec<Arc<DbRow>>> {
        let partitions = self.by_table.remove(table_name)?;

        let mut result = Vec::new();

        for partition in partitions {
            let json = match crate::persist_compression::decompress(&partition.compressed) {
                Ok(json) => json,
                Err(err) => {
                    self.report_broken(table_name, &partition.partition_key, format!("{:?}", err));
                    continue;
                }
            };

            match DbJsonEntity::restore_as_vec(json.as_slice()) {
                Ok(rows) => result.extend(rows),
                Err(err) => {
                    self.report_broken(table_name, &partition.partition_key, format!("{:?}", err));
                }
            }
        }

        Some(result)
    }
}

impl PartitionsInitReader {
    fn report_broken(&self, table_name: &str, partition_key: &str, err: String) {
        if self.skip_errors {
            println!(
                "Can not restore partition Table:{}. PartitionKey: {}. Err: {}",
                table_name, partition_key, err
            );
        } else {
            panic!(
                "Can not restore partition Table:{}. PartitionKey: {}. Err: {}",
                table_name, partition_key, err
            );
        }
    }
}
