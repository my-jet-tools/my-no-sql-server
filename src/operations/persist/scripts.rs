use my_no_sql_sdk::core::db::{DbTableName, PartitionKey};
use my_no_sql_sdk::server::db_snapshots::{DbPartitionSnapshot, DbTableSnapshot};

use std::sync::Arc;

use crate::app::DbNamespace;

pub async fn delete_partition(
    db_namespace: &Arc<DbNamespace>,
    table_name: &DbTableName,
    partition_key: &PartitionKey,
) {
    db_namespace
        .repo
        .delete_partition(table_name, partition_key.as_str())
        .await;
}

pub async fn delete_table(db_namespace: &Arc<DbNamespace>, table_name: &DbTableName) {
    db_namespace.repo.clean_table_content(table_name).await;
    db_namespace.repo.delete_table_metadata(table_name).await;
}

pub async fn sync_table_snapshot(
    db_namespace: &Arc<DbNamespace>,
    table_name: &DbTableName,
    table_snapshot: DbTableSnapshot,
) {
    // Build + compress every partition first (outside any lock), then replace
    // the table content in one repo call that writes the new blobs before
    // removing the partitions that are gone — so a crash mid-sync can never
    // drop a partition that is still part of the table.
    let mut partitions = Vec::with_capacity(table_snapshot.by_partition.len());
    for partition_snapshot in table_snapshot.by_partition {
        let json = partition_snapshot.db_rows_snapshot.as_json_array().build();
        let compressed = crate::persist_compression::compress(json.as_bytes());
        partitions.push((partition_snapshot.partition_key.to_string(), compressed));
    }

    db_namespace
        .repo
        .replace_table_partitions(table_name, partitions)
        .await;
}

pub async fn sync_partition_snapshot(
    db_namespace: &Arc<DbNamespace>,
    table_name: &DbTableName,
    partition_key: &PartitionKey,
    partition_snapshot: DbPartitionSnapshot,
) {
    persist_partition_snapshot(db_namespace, table_name, partition_key, partition_snapshot).await;
}

/// Serializes the whole partition to a JSON array, compresses it and stores it
/// as a single blob. The snapshot is already detached from the DB, so the
/// CPU-heavy JSON build + zstd happen outside any lock (Perf Considerations §6).
async fn persist_partition_snapshot(
    db_namespace: &Arc<DbNamespace>,
    table_name: &DbTableName,
    partition_key: &PartitionKey,
    partition_snapshot: DbPartitionSnapshot,
) {
    let json = partition_snapshot.db_rows_snapshot.as_json_array().build();
    let compressed = crate::persist_compression::compress(json.as_bytes());
    db_namespace
        .repo
        .save_partition(table_name, partition_key.as_str(), &compressed)
        .await;
}
