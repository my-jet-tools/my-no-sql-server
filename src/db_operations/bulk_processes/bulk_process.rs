use std::{collections::BTreeMap, sync::Arc};

use my_no_sql_sdk::core::db::DbNamespaceName;

use my_no_sql_sdk::core::db::DbRow;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;

/// What the committed snapshot does to the table. Fixed when the process is
/// started - the following chunks only carry rows.
///
/// `Table` / `Partition` clean the whole table (or one partition) and re-insert
/// the snapshot. `InsertOrReplaceIfNew` cleans nothing: on commit each row is
/// written only when it is missing or its `TimeStamp` is greater than the stored
/// one (rows must be accumulated keeping their own `TimeStamp`).
#[derive(Debug, Clone)]
pub enum BulkProcessScope {
    Table,
    Partition(String),
    InsertOrReplaceIfNew,
}

/// Rows of a chunked `CleanAndBulkInsert` accumulated aside from the real
/// table. Nothing of it is visible to readers until the commit applies the
/// whole snapshot in one go.
pub struct BulkProcess {
    pub id: String,
    /// Namespace the upload was started in. Kept so the commit can refuse a
    /// request which names a different one instead of cleaning a same-named
    /// table of somebody else.
    pub namespace: DbNamespaceName,
    /// Writer session the process belongs to. `None` for clients which do not
    /// send the `session` header - those get no session binding at all.
    pub session: Option<String>,
    pub table_name: String,
    pub scope: BulkProcessScope,
    pub last_update: DateTimeAsMicroseconds,
    rows_by_partition: BTreeMap<String, Vec<Arc<DbRow>>>,
}

impl BulkProcess {
    pub fn new(
        id: String,
        namespace: DbNamespaceName,
        session: Option<String>,
        table_name: String,
        scope: BulkProcessScope,
        now: DateTimeAsMicroseconds,
    ) -> Self {
        Self {
            id,
            namespace,
            session,
            table_name,
            scope,
            last_update: now,
            rows_by_partition: BTreeMap::new(),
        }
    }

    pub fn append(
        &mut self,
        rows_by_partition: Vec<(String, Vec<Arc<DbRow>>)>,
        now: DateTimeAsMicroseconds,
    ) {
        for (partition_key, db_rows) in rows_by_partition {
            // Partitions are merged across chunks - the same partition may
            // legitimately arrive in several chunks. Duplicated row keys are
            // resolved at insert time by bulk_insert_or_replace: last one wins.
            self.rows_by_partition
                .entry(partition_key)
                .or_insert_with(Vec::new)
                .extend(db_rows);
        }

        self.last_update = now;
    }

    pub fn into_rows_by_partition(self) -> Vec<(String, Vec<Arc<DbRow>>)> {
        self.rows_by_partition.into_iter().collect()
    }
}
