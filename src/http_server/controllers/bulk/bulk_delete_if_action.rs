use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutput};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::app::AppContext;
use crate::db_operations::write::bulk_delete_if::RowToDeleteIf;
use crate::db_sync::EventSource;

use super::models::{
    BulkDeleteIfInputContract, BulkDeleteIfResponseContract, DeleteIfRowContract,
    SkippedDeleteIfRowContract,
};

#[http_route(
    method: "POST",
    route: "/api/Bulk/DeleteIf",
    input_data: "BulkDeleteIfInputContract",
    summary: "Bulk delete of the rows which are still at the given version",
    description: "Deletes a row of the batch only when the TimeStamp stored in the table is still the one sent with that row. A row which has been rewritten meanwhile - and a row which is not there any more - is left alone and reported back in the response, the rest of the batch is still deleted",
    controller: "Bulk",
    result:[
        {status_code: 200, description: "Amount of deleted rows and the ones which were left in place", model: "BulkDeleteIfResponseContract"},
        {status_code: 400, description: "Table not found, or a row of the batch carries no valid TimeStamp"},
    ]
)]
pub struct BulkDeleteIfAction {
    app: Arc<AppContext>,
}

impl BulkDeleteIfAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &BulkDeleteIfAction,
    input_data: BulkDeleteIfInputContract,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let db_table = crate::db_operations::read::table::get(
        action.app.as_ref(),
        &db_namespace,
        input_data.table_name.as_str(),
    )
    .await?;

    let rows_to_delete = group_by_partition_key(input_data.body.deserialize_json()?)?;

    let event_src = EventSource::as_client_request(action.app.as_ref());

    let now = DateTimeAsMicroseconds::now();

    let result = crate::db_operations::write::bulk_delete_if::execute(
        action.app.as_ref(),
        &db_namespace,
        &db_table,
        rows_to_delete,
        event_src,
        input_data.sync_period.get_sync_moment(),
        now,
    )
    .await?;

    let response = BulkDeleteIfResponseContract {
        deleted: result.deleted,
        skipped: result
            .skipped
            .into_iter()
            .map(|itm| SkippedDeleteIfRowContract {
                partition_key: itm.partition_key,
                row_key: itm.row_key,
                reason: itm.reason.as_str().to_string(),
            })
            .collect(),
    };

    HttpOutput::as_json(response).into_ok_result(false).into()
}

/// Groups the flat array the client sends by PartitionKey and turns every `TimeStamp`
/// text into the moment it names - the comparison downstream is between parsed
/// moments, so it does not matter how many fractional digits the value was spelled
/// with.
///
/// A `TimeStamp` which can not be read fails the whole request with 400, naming the
/// offender, the same way the `IfNew` family does. Reporting such a row as merely
/// "skipped" would be worse: an unreadable version can never match a stored one, so a
/// client bug would come back looking exactly like a concurrency conflict.
fn group_by_partition_key(
    rows: Vec<DeleteIfRowContract>,
) -> Result<BTreeMap<String, Vec<RowToDeleteIf>>, HttpFailResult> {
    let mut result: BTreeMap<String, Vec<RowToDeleteIf>> = BTreeMap::new();

    for row in rows {
        let Some(expected_time_stamp) =
            my_no_sql_sdk::abstractions::parse_time_stamp(row.time_stamp.as_str())
        else {
            return Err(HttpFailResult::as_validation_error(format!(
                "Entity with PartitionKey '{}' RowKey '{}' does not contain a valid TimeStamp",
                row.partition_key, row.row_key
            )));
        };

        result
            .entry(row.partition_key)
            .or_default()
            .push(RowToDeleteIf {
                row_key: row.row_key,
                expected_time_stamp,
            });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(partition_key: &str, row_key: &str, time_stamp: &str) -> DeleteIfRowContract {
        DeleteIfRowContract {
            partition_key: partition_key.to_string(),
            row_key: row_key.to_string(),
            time_stamp: time_stamp.to_string(),
        }
    }

    #[test]
    fn rows_of_the_same_partition_end_up_in_one_group() {
        let result = group_by_partition_key(vec![
            row("pk1", "rk1", "2026-08-09T16:44:39.5404"),
            row("pk2", "rk1", "2026-08-09T16:44:39.5404"),
            row("pk1", "rk2", "2026-08-09T16:44:40"),
        ])
        .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result.get("pk1").unwrap().len(), 2);
        assert_eq!(result.get("pk2").unwrap().len(), 1);
    }

    /// Every spelling a `TimeStamp` arrives in has to survive the trip back - a row
    /// read as `...39.5404` and echoed as `...39.540400` is the same version, and the
    /// parsed moment is what the delete compares.
    #[test]
    fn every_spelling_of_the_same_moment_parses_to_the_same_version() {
        let spellings = [
            "2026-08-09T16:44:39.5404",
            "2026-08-09T16:44:39.540400",
            "2026-08-09T16:44:39.540400Z",
            "2026-08-09T16:44:39.5404000",
        ];

        let expected = my_no_sql_sdk::abstractions::parse_time_stamp(spellings[0])
            .unwrap()
            .unix_microseconds;

        for spelling in spellings {
            let result = group_by_partition_key(vec![row("pk", "rk", spelling)]).unwrap();

            assert_eq!(
                result.get("pk").unwrap()[0]
                    .expected_time_stamp
                    .unix_microseconds,
                expected,
                "spelling: {}",
                spelling
            );
        }
    }

    #[test]
    fn an_unreadable_time_stamp_fails_the_whole_request() {
        let result = group_by_partition_key(vec![
            row("pk1", "rk1", "2026-08-09T16:44:39.5404"),
            row("pk1", "rk2", "not-a-time-stamp"),
        ]);

        assert!(result.is_err());
    }
}
