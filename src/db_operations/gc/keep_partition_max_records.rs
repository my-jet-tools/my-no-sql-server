use my_no_sql_sdk::core::db::PartitionKeyParameter;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;
use std::sync::Arc;

use crate::{
    app::{AppContext, DbNamespace},
    db_operations::DbOperationError,
    db_sync::EventSource,
};

pub async fn execute(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: &DbTable,
    partition_key: impl PartitionKeyParameter,
    max_rows_amount: usize,
    event_source: EventSource,
    persist_moment: DateTimeAsMicroseconds,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app)?;

    let rows_to_gc = {
        let table_data = db_table.data.read();
        let partition = table_data.get_partition(partition_key.as_str());

        if partition.is_none() {
            return Ok(());
        }

        let db_rows = partition
            .unwrap()
            .rows
            .get_rows_to_gc_by_max_amount(max_rows_amount);

        if db_rows.is_none() {
            return Ok(());
        }

        db_rows.unwrap()
    };

    super::super::write::bulk_delete(
        app,
        db_namespace,
        db_table,
        [(partition_key, rows_to_gc)].into_iter(),
        event_source,
        persist_moment,
        DateTimeAsMicroseconds::now(),
    )
    .await?;

    Ok(())
}
