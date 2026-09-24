use my_no_sql_sdk::core::db::{DbTableAttributes, DbTableName};

use crate::files_repo::FilesRepo;

use super::{LoadedPartition, LoadedTableAttrs};

/// The persistence backend of a single namespace: a directory of slotted
/// page-files holding one compressed (zstd) blob per partition.
pub struct PersistRepo {
    repo: FilesRepo,
}

impl PersistRepo {
    pub fn new(repo: FilesRepo) -> Self {
        Self { repo }
    }

    pub async fn save_partition(
        &self,
        table_name: &DbTableName,
        partition_key: &str,
        compressed: &[u8],
    ) {
        self.repo
            .save_partition(table_name.as_str(), partition_key, compressed)
            .await
    }

    pub async fn delete_partition(&self, table_name: &DbTableName, partition_key: &str) {
        self.repo
            .delete_partition(table_name.as_str(), partition_key)
            .await
    }

    pub async fn clean_table_content(&self, table_name: &DbTableName) {
        self.repo.clean_table_content(table_name.as_str()).await
    }

    pub async fn save_table_metadata(&self, table_name: &DbTableName, attr: &DbTableAttributes) {
        self.repo
            .save_table_metadata(table_name.as_str(), attr)
            .await
    }

    pub async fn delete_table_metadata(&self, table_name: &DbTableName) {
        self.repo.delete_table_metadata(table_name.as_str()).await
    }

    pub async fn get_tables(&self) -> Vec<LoadedTableAttrs> {
        self.repo.get_tables().await
    }

    pub async fn load_all_partitions(&self, skip_errors: bool) -> Vec<LoadedPartition> {
        self.repo.load_all_partitions(skip_errors).await
    }

    /// Prepares the backend for writes when init skips the normal local-load
    /// path (init-from-other-server): the page-files have to be scanned first —
    /// the scan rebuilds the key index and free-lists and seeds the version
    /// counter past every slot already on disk. Writing into a non-empty
    /// directory without it would append duplicate slots with LOWER versions,
    /// and the next restart's higher-version-wins dedup would revert the whole
    /// import.
    pub async fn prime_for_writes(&self, skip_errors: bool) {
        let _ = self.repo.load_all_partitions(skip_errors).await;
    }

    /// Replaces the entire persisted content of a table with `partitions`
    /// (each `(partition_key, zstd bytes)`), writing the new blobs before
    /// removing any partitions no longer present — so a crash mid-sync cannot
    /// drop a partition that is still part of the table.
    pub async fn replace_table_partitions(
        &self,
        table_name: &DbTableName,
        partitions: Vec<(String, Vec<u8>)>,
    ) {
        self.repo
            .replace_table_partitions(table_name.as_str(), partitions)
            .await
    }

    /// Reclaims page-files whose every slot has been freed (partial files keep
    /// reusing their free slots in place).
    pub async fn vacuum(&self) {
        self.repo.vacuum().await
    }
}
