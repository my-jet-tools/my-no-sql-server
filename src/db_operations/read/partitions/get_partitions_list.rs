use std::sync::Arc;

use my_no_sql_sdk::server::DbTable;

use crate::{app::AppContext, db_operations::DbOperationError};

pub async fn get_partitions(
    app: &AppContext,
    db_table: &Arc<DbTable>,
    limit: Option<usize>,
    skip: Option<usize>,
) -> Result<(usize, Vec<String>), DbOperationError> {
    super::super::super::check_app_states(app)?;

    let table_data = db_table.data.read();

    let count = table_data.partitions.len();

    let items = table_data.partitions.get_partitions();

    let result = crate::db_operations::read::read_filter::filter_it(items, limit, skip);
    Ok((
        count,
        result
            .iter()
            .map(|itm| itm.partition_key.to_string())
            .collect(),
    ))
}
