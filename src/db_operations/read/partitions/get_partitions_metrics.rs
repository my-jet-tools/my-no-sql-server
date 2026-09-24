use std::sync::Arc;

use my_no_sql_sdk::server::DbTable;

use crate::{app::AppContext, db_operations::DbOperationError};

pub struct DbPartitionMetric {
    pub partition_key: String,
    pub records_count: usize,
    pub data_size: usize,
}

/// Returns per-partition metrics (records count and content size in bytes) for
/// every partition of the table, in the table's natural partition order. Both
/// values are read from cheap O(1) accessors on the partition, so the whole
/// scan is O(partitions).
pub async fn get_partitions_metrics(
    app: &AppContext,
    db_table: &Arc<DbTable>,
) -> Result<Vec<DbPartitionMetric>, DbOperationError> {
    super::super::super::check_app_states(app)?;

    let table_data = db_table.data.read();

    let result = table_data
        .partitions
        .get_partitions()
        .map(|partition| DbPartitionMetric {
            partition_key: partition.partition_key.to_string(),
            records_count: partition.rows_count(),
            data_size: partition.get_content_size(),
        })
        .collect();

    Ok(result)
}
