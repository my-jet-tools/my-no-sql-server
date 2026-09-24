use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{
    app::{AppContext, DbNamespace},
    persist_markers::PersistTask,
};

/// Writes one queued task and returns whether it wrote anything.
///
/// A task which is not due yet is left alone, unless the server is shutting
/// down and everything has to go out now.
pub async fn persist(app: &Arc<AppContext>) -> bool {
    let now = if app.states.is_shutting_down() {
        None
    } else {
        Some(DateTimeAsMicroseconds::now())
    };

    persist_task(app, now).await
}

/// Writes everything queued and returns how many tasks that took.
///
/// Answers only when the data is on the disk — this is what a caller uses as a
/// barrier before it restarts the process. Tasks scheduled for later are written
/// too: the point of asking is that the data is safe now, not that it is due.
pub async fn persist_all(app: &Arc<AppContext>) -> usize {
    let mut persisted = 0;

    while persist_task(app, None).await {
        persisted += 1;
    }

    persisted
}

/// How many writes the server still owes the disk, across every namespace.
///
/// Read live: an operator asks it to find out whether a restart is safe, and an
/// answer a second old is an answer about the wrong moment.
pub async fn get_amount_to_persist(app: &AppContext) -> usize {
    let mut result = 0;

    for db_namespace in app.namespaces.get_all() {
        result += db_namespace.persist_markers.get_amount_to_persist().await;
    }

    result
}

async fn persist_task(app: &Arc<AppContext>, now: Option<DateTimeAsMicroseconds>) -> bool {
    // Single-flight: the timer, the Force-Persist HTTP action and the shutdown
    // drain may call this concurrently; overlapping tasks would break the
    // in-flight-write-then-cleanup ordering that table deletion relies on.
    //
    // One lock for the whole server rather than one per namespace: that ordering
    // is what makes a table delete safe, and walking the namespaces is cheap.
    let _single_flight = app.persist_call_lock.lock().await;

    let start_time = DateTimeAsMicroseconds::now();

    // Each namespace keeps a persist queue of its own. Take the first task we
    // find and let the caller come back for more — one task per call keeps every
    // namespace moving instead of draining one of them under a time budget.
    for db_namespace in app.namespaces.get_all() {
        let persist_task = match db_namespace.persist_markers.get_persist_task(now).await {
            Some(persist_task) => persist_task,
            None => continue,
        };

        execute_persist_task(&db_namespace, persist_task, start_time).await;

        return true;
    }

    false
}

async fn execute_persist_task(
    db_namespace: &Arc<DbNamespace>,
    persist_task: PersistTask,
    start_time: DateTimeAsMicroseconds,
) {
    let db_table_name = match persist_task {
        PersistTask::SaveTableAttributes(db_table_name) => {
            super::save_table_attributes(db_namespace, &db_table_name).await;
            db_table_name
        }
        PersistTask::SyncTable(db_table_name) => {
            super::save_table(db_namespace, &db_table_name).await;
            db_table_name
        }
        PersistTask::SyncPartition {
            table_name,
            partition_key,
        } => {
            super::save_partition(db_namespace, &table_name, partition_key).await;
            table_name
        }
        PersistTask::SyncRows { table_name, jobs } => {
            super::save_rows(db_namespace, &table_name, jobs).await;
            table_name
        }
    };

    let now = DateTimeAsMicroseconds::now();
    let duration = now.duration_since(start_time).as_positive_or_zero();

    db_namespace
        .persist_markers
        .set_last_persist_time(&db_table_name, now, duration)
        .await;
}
