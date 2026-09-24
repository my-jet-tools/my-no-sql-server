use my_no_sql_sdk::core::db::{DbTableAttributes, DbTableName};

use crate::operations::init::TableAttributeInitContract;

/// Backend-neutral table descriptor returned by `PersistRepo::get_tables`.
/// Produced by the Files backend, so the init path
/// (`init_tables`) is backend-agnostic.
pub struct LoadedTableAttrs {
    pub table_name: DbTableName,
    pub attr: DbTableAttributes,
}

impl TableAttributeInitContract for LoadedTableAttrs {
    fn into(self) -> (DbTableName, DbTableAttributes) {
        (self.table_name, self.attr)
    }
}
