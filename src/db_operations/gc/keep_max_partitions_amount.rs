use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::{AppContext, DbNamespace},
    db_operations::DbOperationError,
    db_sync::EventSource,
};

pub async fn keep_max_partitions_amount(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: &Arc<DbTable>,
    max_partitions_amount: usize,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app)?;

    let partitions_to_gc = {
        let read_access = db_table.data.read();

        read_access
            .partitions
            .get_partitions_to_gc_by_max_amount(max_partitions_amount)
    };

    if let Some(partitions_to_gc) = partitions_to_gc {
        super::super::write::delete_partitions(
            app,
            db_namespace,
            db_table,
            partitions_to_gc.into_iter().map(|itm| itm.partition_key),
            event_src,
            persist_moment,
            DateTimeAsMicroseconds::now(),
        )
        .await?;
    }

    Ok(())
}
