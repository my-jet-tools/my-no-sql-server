use my_no_sql_sdk::core::{
    db::DbRow,
    db_json_entity::{DbJsonEntity, DbJsonEntityWithContent, JsonTimeStamp},
};

use crate::db_operations::DbOperationError;

pub fn parse_db_json_entity(src: &[u8], now: &JsonTimeStamp) -> Result<DbRow, DbOperationError> {
    match DbJsonEntity::parse_into_db_row(src.into(), now) {
        Ok(result) => Ok(result),
        Err(err) => {
            let result = DbOperationError::DbEntityParseFail(err);
            Err(result)
        }
    }
}

/// Same as [`parse_db_json_entity`], but the parsed row keeps the `TimeStamp` that
/// came in the entity. Unlike the plain variant it does NOT substitute the server
/// clock: an entity without a valid `TimeStamp` is an error (naming that entity).
/// Used by `insert_or_replace_if_new` to compare against the timestamp already stored
/// in the table.
pub fn parse_db_json_entity_and_keep_date_time(src: &[u8]) -> Result<DbRow, DbOperationError> {
    match DbJsonEntity::parse_into_db_row_and_keep_date_time(src.into()) {
        Ok(result) => Ok(result),
        Err(err) => {
            let result = DbOperationError::DbEntityParseFail(err);
            Err(result)
        }
    }
}

pub fn parse_db_json_entity_to_validate<'s>(
    src: &'s [u8],
    now: &'s JsonTimeStamp,
) -> Result<DbJsonEntityWithContent<'s>, DbOperationError> {
    match DbJsonEntity::parse(src, now) {
        Ok(result) => Ok(result),
        Err(err) => {
            let result = DbOperationError::DbEntityParseFail(err);
            Err(result)
        }
    }
}
