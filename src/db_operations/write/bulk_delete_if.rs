use crate::app::DbNamespace;
use std::collections::BTreeMap;
use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::{
    app::AppContext,
    db_operations::DbOperationError,
    db_sync::{states::DeleteRowsEventSyncData, EventSource, SyncEvent},
};

/// One row the client asks to delete, together with the version it read that row at.
pub struct RowToDeleteIf {
    pub row_key: String,
    pub expected_time_stamp: DateTimeAsMicroseconds,
}

/// Why a requested row is still in the table after the operation.
pub enum DeleteIfSkipReason {
    /// There is no such PartitionKey/RowKey - somebody deleted it already, or it
    /// never existed.
    NotFound,
    /// The row is there, but it is not the version the client read it at.
    TimeStampMismatch,
}

impl DeleteIfSkipReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeleteIfSkipReason::NotFound => "NotFound",
            DeleteIfSkipReason::TimeStampMismatch => "TimeStampMismatch",
        }
    }
}

pub struct SkippedRow {
    pub partition_key: String,
    pub row_key: String,
    pub reason: DeleteIfSkipReason,
}

pub struct BulkDeleteIfResult {
    /// How many rows really left the table.
    pub deleted: usize,
    /// The requested rows which were left in place, each with the reason why.
    pub skipped: Vec<SkippedRow>,
}

/// Bulk delete guarded by optimistic concurrency: a row goes only when it is still
/// the version the client read it at. A row whose stored `TimeStamp` differs - or
/// which is not there at all - is left alone and reported back in
/// [`BulkDeleteIfResult::skipped`]; the rest of the batch is still deleted. This is
/// the partial-success counterpart of the single-row `delete_row_if`, which answers
/// the same two situations with 404 / 409.
///
/// Versions are compared as parsed moments (microseconds since epoch), never as text,
/// so how many fractional digits the `TimeStamp` was spelled with does not matter -
/// the same rule the `replace` operation follows and tests.
pub async fn execute(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: &Arc<DbTable>,
    rows_to_delete: BTreeMap<String, Vec<RowToDeleteIf>>,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
    now: DateTimeAsMicroseconds,
) -> Result<BulkDeleteIfResult, DbOperationError> {
    super::super::check_app_states(app)?;

    enum PersistOp {
        Partition(my_no_sql_sdk::core::db::PartitionKey),
        Rows(
            my_no_sql_sdk::core::db::PartitionKey,
            Vec<Arc<my_no_sql_sdk::core::db::DbRow>>,
        ),
    }

    let (sync_data, persist_ops, result) = {
        let mut table_data = db_table.data.write();
        let mut sync_data = DeleteRowsEventSyncData::new(&table_data, event_src);
        let mut persist_ops: Vec<PersistOp> = Vec::new();

        let mut deleted = 0;
        let mut skipped: Vec<SkippedRow> = Vec::new();

        for (partition_key, rows) in rows_to_delete {
            // Which of the requested rows still carry the version they were read at.
            // The check and the removal below happen under the same write lock - a
            // read lock first would leave a window for somebody to rewrite a row in
            // between the two.
            let row_keys_to_delete: Vec<String> =
                match table_data.get_partition(partition_key.as_str()) {
                    Some(db_partition) => {
                        let mut matched = Vec::with_capacity(rows.len());

                        for row in rows {
                            let reason = match db_partition.get_row(row.row_key.as_str()) {
                                Some(db_row) => {
                                    if db_row.get_time_stamp_as_date_time().unix_microseconds
                                        == row.expected_time_stamp.unix_microseconds
                                    {
                                        matched.push(row.row_key);
                                        continue;
                                    }

                                    DeleteIfSkipReason::TimeStampMismatch
                                }
                                None => DeleteIfSkipReason::NotFound,
                            };

                            skipped.push(SkippedRow {
                                partition_key: partition_key.clone(),
                                row_key: row.row_key,
                                reason,
                            });
                        }

                        matched
                    }
                    None => {
                        for row in rows {
                            skipped.push(SkippedRow {
                                partition_key: partition_key.clone(),
                                row_key: row.row_key,
                                reason: DeleteIfSkipReason::NotFound,
                            });
                        }

                        Vec::new()
                    }
                };

            if row_keys_to_delete.is_empty() {
                continue;
            }

            let removed_rows_result = table_data.bulk_remove_rows(
                &partition_key,
                row_keys_to_delete.into_iter(),
                true,
                Some(now),
            );

            if let Some((partition_key, removed_rows, partition_is_empty)) = removed_rows_result {
                deleted += removed_rows.len();

                if partition_is_empty {
                    sync_data.new_deleted_partition(&partition_key);
                    persist_ops.push(PersistOp::Partition(partition_key));
                } else {
                    sync_data.add_deleted_rows(&partition_key, &removed_rows);
                    persist_ops.push(PersistOp::Rows(partition_key, removed_rows));
                }
            }
        }

        (sync_data, persist_ops, BulkDeleteIfResult { deleted, skipped })
    };

    // Nothing matched - no persistence to schedule and nothing to tell the readers about.
    if persist_ops.is_empty() {
        return Ok(result);
    }

    for op in persist_ops {
        match op {
            PersistOp::Partition(pk) => {
                db_namespace
                    .persist_markers
                    .persist_partition(&db_table.name, &pk, persist_moment)
                    .await;
            }
            PersistOp::Rows(pk, rows) => {
                db_namespace
                    .persist_markers
                    .delete_db_rows(&db_table.name, &pk, persist_moment, rows.iter())
                    .await;
            }
        }
    }

    crate::operations::sync::dispatch(app, db_namespace, SyncEvent::DeleteRows(sync_data));

    Ok(result)
}
