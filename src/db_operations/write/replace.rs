use std::sync::Arc;

use crate::{
    app::{AppContext, DbNamespace},
    db_operations::DbOperationError,
    db_sync::{states::UpdateRowsSyncData, EventSource, SyncEvent},
};

use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::core::{
    db::DbRow,
    db_json_entity::{DbJsonEntityWithContent, JsonTimeStamp},
};
use my_no_sql_sdk::server::DbTable;

use super::WriteOperationResult;

/// What [`validate_before`] hands over to [`execute`].
///
/// `db_row` is the row we are about to write - it already carries the **server clock**
/// as its `TimeStamp` (`parse_into_db_row` overwrites whatever the client sent).
/// `expected_time_stamp` is the version the client read the row at, parsed from the
/// `TimeStamp` of the incoming entity. The two are different moments and must never be
/// mixed up: the optimistic-concurrency check compares the **stored** row against
/// `expected_time_stamp`, never against the new row's freshly stamped value.
pub struct ReplaceCandidate {
    pub db_row: DbRow,
    pub expected_time_stamp: DateTimeAsMicroseconds,
}

/// The version comparison behind the optimistic-concurrency check.
///
/// Both sides are compared as parsed moments (microseconds since epoch), never as text,
/// so how many fractional digits the `TimeStamp` was spelled with does not matter:
/// `...:39.5404`, `...:39.540400` and `...:39.540400Z` are the same version.
#[inline]
fn same_version(stored: DateTimeAsMicroseconds, expected: DateTimeAsMicroseconds) -> bool {
    stored.unix_microseconds == expected.unix_microseconds
}

#[inline]
pub async fn validate_before(
    app: &AppContext,
    db_table: &Arc<DbTable>,
    db_entity: DbJsonEntityWithContent<'_>,
) -> Result<ReplaceCandidate, DbOperationError> {
    super::super::check_app_states(app)?;

    if db_entity.get_time_stamp().is_none() {
        return Err(DbOperationError::TimeStampFieldRequires);
    }

    let incoming_time_stamp =
        DateTimeAsMicroseconds::parse_iso_string(db_entity.get_time_stamp().unwrap());

    let Some(expected_time_stamp) = incoming_time_stamp else {
        return Err(DbOperationError::OptimisticConcurrencyUpdateFails);
    };

    {
        let read_access = db_table.data.read();

        let db_partition = read_access.get_partition(db_entity.get_partition_key());

        if db_partition.is_none() {
            return Err(DbOperationError::RecordNotFound);
        }

        let db_row = db_partition.unwrap().get_row(db_entity.get_row_key());

        if db_row.is_none() {
            return Err(DbOperationError::RecordNotFound);
        }

        if !same_version(
            db_row.unwrap().get_time_stamp_as_date_time(),
            expected_time_stamp,
        ) {
            return Err(DbOperationError::OptimisticConcurrencyUpdateFails);
        }
    }

    Ok(ReplaceCandidate {
        db_row: db_entity.into_db_row()?,
        expected_time_stamp,
    })
}

pub async fn execute(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: &Arc<DbTable>,
    db_row: Arc<DbRow>,
    expected_time_stamp: DateTimeAsMicroseconds,
    event_src: EventSource,

    persist_moment: DateTimeAsMicroseconds,
    now: &JsonTimeStamp,
) -> Result<WriteOperationResult, DbOperationError> {
    let (partition_key, update_rows_state) = {
        let mut table_data = db_table.data.write();

        let partition_key = {
            let db_partition = table_data.get_partition_mut(db_row.get_partition_key());

            if db_partition.is_none() {
                return Err(DbOperationError::RecordNotFound);
            }

            let db_partition = db_partition.unwrap();

            let current_db_row = db_partition.get_row(db_row.get_row_key());

            match current_db_row {
                Some(current_db_row) => {
                    // Re-check under the write lock: `validate_before` only held a read
                    // lock, so somebody could have replaced the row in between. The
                    // version to compare against is the one the client read the row at -
                    // `db_row` here is the *new* row and already carries the server clock.
                    if !same_version(
                        current_db_row.get_time_stamp_as_date_time(),
                        expected_time_stamp,
                    ) {
                        return Err(DbOperationError::OptimisticConcurrencyUpdateFails);
                    }
                }
                None => {
                    return Err(DbOperationError::RecordNotFound);
                }
            }

            db_partition.partition_key.clone()
        };

        table_data.remove_row(&partition_key, &db_row, false, None);
        table_data.insert_row(&db_row, Some(now.date_time));

        let mut update_rows_state = UpdateRowsSyncData::new(&table_data, event_src);
        update_rows_state
            .rows_by_partition
            .add_row(partition_key.clone(), db_row.clone());

        (partition_key, update_rows_state)
    };

    db_namespace
        .persist_markers
        .persist_rows(
            &db_table.name,
            &partition_key,
            persist_moment,
            [&db_row].into_iter(),
        )
        .await;

    crate::operations::sync::dispatch(app, db_namespace, SyncEvent::UpdateRows(update_rows_state));

    Ok(WriteOperationResult::SingleRow(db_row))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How a stored row derives its version: `DbRowPlain::new` parses the `TimeStamp`
    /// text out of the row's own JSON.
    fn stored_version(time_stamp_in_the_row: &str) -> DateTimeAsMicroseconds {
        my_no_sql_sdk::abstractions::parse_time_stamp(time_stamp_in_the_row).unwrap()
    }

    /// How a client sends the version back: `my-no-sql-abstractions`' `Timestamp` parses
    /// the text it read, keeps the microseconds, and re-serializes them with its own
    /// spelling. The real functions are called here, not a copy of them, so the test
    /// cannot drift away from what the client actually puts on the wire.
    fn what_the_client_sends_back(time_stamp_in_the_row: &str) -> String {
        my_no_sql_sdk::abstractions::format_time_stamp(stored_version(time_stamp_in_the_row))
    }

    /// The whole read -> send back -> compare loop, exactly as the server runs it.
    fn read_modify_write_is_accepted(time_stamp_in_the_row: &str) -> bool {
        let incoming = what_the_client_sends_back(time_stamp_in_the_row);
        let incoming = DateTimeAsMicroseconds::parse_iso_string(incoming.as_str()).unwrap();
        same_version(stored_version(time_stamp_in_the_row), incoming)
    }

    /// Every fractional-digit width the `TimeStamp` can be spelled with - including the
    /// 4-digit one rows written before this fix carry on disk (`to_rfc3339()` renders
    /// trailing zeros, `JsonTimeStamp` cuts them off) - survives a read -> replace
    /// round-trip untouched. Text width never enters the comparison; the parsed moment does.
    #[test]
    fn round_trip_of_an_untouched_row_is_accepted_for_any_fraction_width() {
        for time_stamp in [
            "2026-08-09T16:44:39",         // 0 digits
            "2026-08-09T16:44:39.5",       // 1
            "2026-08-09T16:44:39.540",     // 3
            "2026-08-09T16:44:39.5404",    // 4 - what a pre-fix row looks like
            "2026-08-09T16:44:39.540400",  // 6
            "2026-08-09T16:44:39.5404001", // 7
        ] {
            assert!(
                read_modify_write_is_accepted(time_stamp),
                "replace of an untouched row must be accepted for {}",
                time_stamp
            );
        }
    }

    /// Same, with the UTC zone spelled out. `Z` and `+00:00` are the two spellings that
    /// reach the server (`to_rfc3339_utc()` and the older `to_rfc3339()`).
    #[test]
    fn round_trip_is_accepted_with_the_utc_zone_spelled_out() {
        for suffix in ["", "Z", "+00:00"] {
            for fraction in ["", ".5", ".540", ".5404", ".540400", ".5404001"] {
                let time_stamp = format!("2026-08-09T16:44:39{}{}", fraction, suffix);
                assert!(
                    read_modify_write_is_accepted(time_stamp.as_str()),
                    "replace of an untouched row must be accepted for {}",
                    time_stamp
                );
            }
        }
    }

    /// The same moment written with different widths is the same version - this is what
    /// makes a row stored before the fix (4 digits) accept a client that sends 6 back.
    #[test]
    fn the_same_moment_spelled_differently_is_the_same_version() {
        let stored = stored_version("2026-08-09T16:44:39.5404");

        for incoming in [
            "2026-08-09T16:44:39.5404",
            "2026-08-09T16:44:39.54040",
            "2026-08-09T16:44:39.540400",
            "2026-08-09T16:44:39.540400Z",
            "2026-08-09T16:44:39.5404000",
        ] {
            let incoming = DateTimeAsMicroseconds::parse_iso_string(incoming).unwrap();
            assert!(same_version(stored, incoming), "{} must match", incoming);
        }
    }

    /// And the guarantee the whole thing exists for: somebody really did rewrite the row
    /// between the read and the write, so the versions differ and the write is refused.
    /// A microsecond apart is enough - the server stamps rows with microsecond precision.
    #[test]
    fn a_real_conflict_is_still_refused() {
        let read_at = stored_version("2026-08-09T16:44:39.540400");

        for rewritten_at in [
            "2026-08-09T16:44:39.540401", // one microsecond later
            "2026-08-09T16:44:39.541400", // one millisecond later
            "2026-08-09T16:44:40.540400", // one second later
            "2026-08-09T16:44:39.540399", // and going backwards is a conflict too
        ] {
            let rewritten_at = stored_version(rewritten_at);
            assert!(
                !same_version(rewritten_at, read_at),
                "a row rewritten at {} must not accept a write based on {}",
                rewritten_at.to_rfc3339(),
                read_at.to_rfc3339()
            );
        }
    }
}
