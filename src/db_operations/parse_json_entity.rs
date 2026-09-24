use std::sync::Arc;

use my_no_sql_sdk::core::{
    db::DbRow,
    db_json_entity::{DbJsonEntity, JsonTimeStamp},
};

use super::DbOperationError;

pub fn parse_grouped_by_partition_key(
    as_bytes: &[u8],
    inject_time_stamp: &JsonTimeStamp,
) -> Result<Vec<(String, Vec<Arc<DbRow>>)>, DbOperationError> {
    match DbJsonEntity::parse_grouped_by_partition_key(as_bytes, inject_time_stamp) {
        Ok(result) => Ok(result),
        Err(err) => {
            let result = DbOperationError::DbEntityParseFail(err);
            Err(result)
        }
    }
}

/// Same as [`parse_grouped_by_partition_key`], but each parsed row keeps the
/// `TimeStamp` that came in the entity. Unlike the plain variant it does NOT
/// substitute the server clock: an entity without a valid `TimeStamp` is an error
/// (naming that entity). This is what the `*_if_new` operations compare against the
/// timestamp already stored in the table.
pub fn parse_grouped_by_partition_key_and_keep_date_time(
    as_bytes: &[u8],
) -> Result<Vec<(String, Vec<Arc<DbRow>>)>, DbOperationError> {
    match DbJsonEntity::parse_grouped_by_partition_key_and_keep_date_time(as_bytes) {
        Ok(result) => Ok(result),
        Err(err) => {
            let result = DbOperationError::DbEntityParseFail(err);
            Err(result)
        }
    }
}

/// Picks the parsing behaviour from the `useTimestamp` request flag:
/// - `Some(true)` → keep each entity's own `TimeStamp` (every entity must carry a
///   valid one, otherwise it fails naming the offender);
/// - `None` / `Some(false)` → substitute the server clock (`inject_time_stamp`) for
///   every row, as before.
pub fn parse_grouped_by_partition_key_with_use_timestamp(
    as_bytes: &[u8],
    use_timestamp: Option<bool>,
    inject_time_stamp: &JsonTimeStamp,
) -> Result<Vec<(String, Vec<Arc<DbRow>>)>, DbOperationError> {
    if use_timestamp == Some(true) {
        parse_grouped_by_partition_key_and_keep_date_time(as_bytes)
    } else {
        parse_grouped_by_partition_key(as_bytes, inject_time_stamp)
    }
}
