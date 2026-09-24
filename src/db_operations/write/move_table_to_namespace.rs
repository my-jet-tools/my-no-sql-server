use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{
    app::{AppContext, DbNamespace},
    db_operations::DbOperationError,
    db_sync::{
        states::{DeleteTableSyncData, InitTableEventSyncData},
        EventSource, SyncEvent,
    },
};

/// Moves a whole table from one namespace to another.
///
/// The table object itself is handed over as it is — rows are not copied, the
/// same `Arc<DbTable>` simply stops being reachable through the source namespace
/// and starts being reachable through the destination one. What does change is
/// where it persists: the destination gets a full content sync into its own
/// folder, and the source gets the same cleanup a table delete would do.
pub async fn move_table_to_namespace(
    app: &Arc<AppContext>,
    from: &Arc<DbNamespace>,
    to: &Arc<DbNamespace>,
    table_name: &str,
    event_src: EventSource,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app.as_ref())?;

    if from.name == to.name {
        return Err(DbOperationError::TableNameValidationError(format!(
            "Table '{}' is already in the namespace '{}'",
            table_name, to.name
        )));
    }

    // Refuse rather than overwrite: the destination table would be silently
    // replaced together with every row in it.
    if to.db.get_table(table_name).is_some() {
        return Err(DbOperationError::TableAlreadyExists);
    }

    let db_table = match from.db.get_table(table_name) {
        Some(db_table) => db_table,
        None => return Err(DbOperationError::TableNotFound(table_name.to_string())),
    };

    // Publish into the destination BEFORE removing from the source: a reader or
    // a writer racing this move then finds the table in one namespace or in the
    // other, never in neither.
    to.db.insert(db_table.clone());

    let removed = from.db.delete_table(table_name);

    if removed.is_none() {
        // Somebody deleted it between the lookup and here. Undo the publish so
        // the move does not resurrect a table the other caller just dropped.
        to.db.delete_table(table_name);
        return Err(DbOperationError::TableNotFound(table_name.to_string()));
    }

    let now = DateTimeAsMicroseconds::now();

    let (init_sync_data, delete_sync_data, db_table_name) = {
        let table_data = db_table.data.read();
        (
            InitTableEventSyncData::new(&table_data, event_src.clone()),
            DeleteTableSyncData::new(&table_data, event_src),
            table_data.name.clone(),
        )
    };

    // Destination first: its attributes and its whole content have to reach the
    // new folder. Only then the source cleanup, which is the same two markers a
    // delete uses — the content marker is what removes the blobs and the
    // metadata from the source folder once the persist loop sees the table gone.
    to.persist_markers
        .persist_table_attributes(&db_table_name, now)
        .await;

    to.persist_markers
        .persist_table_content(&db_table_name, now)
        .await;

    from.persist_markers
        .persist_table_attributes(&db_table_name, now)
        .await;

    from.persist_markers
        .persist_table_content(&db_table_name, now)
        .await;

    // Subscribers of the source namespace lose the table; subscribers of the
    // destination namespace get it initialized.
    crate::operations::sync::dispatch(app.as_ref(), from, SyncEvent::DeleteTable(delete_sync_data));

    crate::operations::sync::dispatch(app.as_ref(), to, SyncEvent::InitTable(init_sync_data));

    Ok(())
}
