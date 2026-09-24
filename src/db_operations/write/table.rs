use std::sync::Arc;

use my_no_sql_sdk::core::db::{DbTableAttributes, DbTableInner};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use my_no_sql_sdk::server::DbTable;

use crate::db_operations::validation;
use crate::{
    app::{AppContext, DbNamespace},
    db_operations::DbOperationError,
    db_sync::{
        states::{
            DeleteTableSyncData, InitTableEventSyncData, SyncTableData,
            UpdateTableAttributesSyncData,
        },
        EventSource, SyncEvent,
    },
};

pub async fn create(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    table_name: &str,
    persist_table: bool,
    max_partitions_amount: Option<usize>,
    max_rows_per_partition_amount: Option<usize>,
    compressed: bool,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
) -> Result<Arc<DbTable>, DbOperationError> {
    super::super::check_app_states(app)?;

    validation::validate_table_name(table_name)?;

    let now = DateTimeAsMicroseconds::now();

    let create_table_result = get_or_create_table(
        app,
        db_namespace,
        table_name,
        persist_table,
        max_partitions_amount,
        max_rows_per_partition_amount,
        compressed,
        now,
    )
    .await;

    match create_table_result {
        CreateTableResult::JustCreated(db_table) => {
            let (state, table_name) = {
                let table_data = db_table.data.read();
                (
                    InitTableEventSyncData::new(&table_data, event_src),
                    table_data.name.clone(),
                )
            };

            db_namespace
                .persist_markers
                .persist_table_attributes(&table_name, persist_moment)
                .await;

            db_namespace
                .persist_markers
                .persist_table_attributes(&table_name, persist_moment)
                .await;

            crate::operations::sync::dispatch(app, db_namespace, SyncEvent::InitTable(state));

            return Ok(db_table);
        }
        CreateTableResult::AlreadyHadTable(_) => {
            return Err(DbOperationError::TableAlreadyExists);
        }
    }
}

async fn get_or_create(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    table_name: &str,
    persist_table: bool,
    max_partitions_amount: Option<usize>,
    max_rows_per_partition_amount: Option<usize>,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
) -> Result<Arc<DbTable>, DbOperationError> {
    validation::validate_table_name(table_name)?;
    let now = DateTimeAsMicroseconds::now();

    let create_table_result = get_or_create_table(
        app,
        db_namespace,
        table_name,
        persist_table,
        max_partitions_amount,
        max_rows_per_partition_amount,
        false,
        now,
    )
    .await;

    match create_table_result {
        CreateTableResult::JustCreated(db_table) => {
            let (state, table_name) = {
                let table_data = db_table.data.read();
                (
                    InitTableEventSyncData::new(&table_data, event_src),
                    table_data.name.clone(),
                )
            };

            crate::operations::sync::dispatch(app, db_namespace, SyncEvent::InitTable(state));

            db_namespace
                .persist_markers
                .persist_table_attributes(&table_name, persist_moment)
                .await;

            return Ok(db_table);
        }
        CreateTableResult::AlreadyHadTable(db_table) => {
            return Ok(db_table);
        }
    }
}

pub async fn create_if_not_exist(
    app: &Arc<AppContext>,
    db_namespace: &Arc<DbNamespace>,
    table_name: &str,
    persist_table: bool,
    max_partitions_amount: Option<usize>,
    max_rows_per_partition_amount: Option<usize>,
    event_src: EventSource,

    persist_moment: DateTimeAsMicroseconds,
) -> Result<Arc<DbTable>, DbOperationError> {
    super::super::check_app_states(app)?;

    validation::validate_table_name(table_name)?;

    let db_table = get_or_create(
        app,
        db_namespace,
        table_name,
        persist_table,
        max_partitions_amount,
        max_rows_per_partition_amount,
        event_src.clone(),
        persist_moment,
    )
    .await?;

    set_table_attributes(
        app,
        db_namespace,
        db_table.clone(),
        persist_table,
        max_partitions_amount,
        max_rows_per_partition_amount,
        event_src,
    )
    .await?;

    Ok(db_table)
}

pub async fn update_persist_state(
    app: &Arc<AppContext>,
    db_namespace: &Arc<DbNamespace>,
    db_table: Arc<DbTable>,
    persist: bool,
    event_src: EventSource,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app)?;
    let attrs = db_table.get_attributes();

    set_table_attributes(
        app,
        db_namespace,
        db_table,
        persist,
        attrs.max_partitions_amount,
        attrs.max_rows_per_partition_amount,
        event_src,
    )
    .await?;
    Ok(())
}

pub async fn update_compressed_state(
    app: &Arc<AppContext>,
    db_namespace: &Arc<DbNamespace>,
    db_table: Arc<DbTable>,
    compressed: bool,
    force_compress: bool,
    event_src: EventSource,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app)?;

    let (flag_changed, table_name) = {
        let mut write_access = db_table.data.write();
        // By default this is lazy: only flip the flag. Existing rows stay as they are;
        // new / rewritten rows get compressed (or not) at the insert choke point. A
        // table may hold a mix of plain and compressed rows — they render identically.
        let flag_changed = write_access.attributes.set_compressed(compressed);
        // force_compress: immediately re-encode every already stored row to match the
        // current flag (compress all if on, decompress all if off). CPU-heavy on big
        // tables — opt-in only.
        if force_compress {
            write_access.apply_rows_compression();
        }
        (flag_changed, write_access.name.clone())
    };

    // Nothing to persist/sync if the flag did not change (rows persist as plain JSON
    // and readers always receive plain JSON, so a forced re-encode alone is invisible).
    if !flag_changed {
        return Ok(());
    }

    crate::operations::sync::dispatch(
        app,
        db_namespace,
        SyncEvent::UpdateTableAttributes(UpdateTableAttributesSyncData {
            table_data: SyncTableData {
                table_name: db_table.name.clone(),
            },
            event_src,
        }),
    );

    db_namespace
        .persist_markers
        .persist_table_attributes(&table_name, DateTimeAsMicroseconds::now())
        .await;

    Ok(())
}

pub async fn set_table_attributes(
    app: &Arc<AppContext>,
    db_namespace: &Arc<DbNamespace>,
    db_table: Arc<DbTable>,
    persist: bool,
    max_partitions_amount: Option<usize>,
    max_rows_per_partition_amount: Option<usize>,
    event_src: EventSource,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app)?;

    let (updated, table_name) = {
        let mut write_access = db_table.data.write();
        let updated = write_access.attributes.update(
            persist,
            max_partitions_amount,
            max_rows_per_partition_amount,
        );
        (updated, write_access.name.clone())
    };

    if !updated {
        return Ok(());
    }

    crate::operations::sync::dispatch(
        app,
        db_namespace,
        SyncEvent::UpdateTableAttributes(UpdateTableAttributesSyncData {
            table_data: SyncTableData {
                table_name: db_table.name.clone(),
            },
            event_src,
        }),
    );

    db_namespace
        .persist_markers
        .persist_table_attributes(&table_name, DateTimeAsMicroseconds::now())
        .await;

    Ok(())
}

pub async fn delete(
    app: Arc<AppContext>,
    db_namespace: Arc<DbNamespace>,
    table_name: String,
    event_src: EventSource,
    persist_moment: DateTimeAsMicroseconds,
) -> Result<(), DbOperationError> {
    super::super::check_app_states(app.as_ref())?;
    let result = db_namespace.db.delete_table(table_name.as_str());

    if result.is_none() {
        return Err(DbOperationError::TableNotFound(table_name));
    }

    let db_table = result.unwrap();

    let (sync_data, table_name) = {
        let table_data = db_table.data.read();
        (
            DeleteTableSyncData::new(&table_data, event_src),
            table_data.name.clone(),
        )
    };

    db_namespace
        .persist_markers
        .persist_table_attributes(&table_name, persist_moment)
        .await;

    // Repo cleanup must ride the serialized persist loop, not a detached task:
    // a SyncPartition task already in flight for this table lands its
    // save_partition first, and only then the SyncTable task below sees the
    // table gone and deletes its content + metadata — so the cleanup can never
    // be overtaken by a stale in-flight blob. If the table is recreated before
    // the task fires, the same task becomes a full re-sync that prunes the old
    // partitions instead.
    db_namespace
        .persist_markers
        .persist_table_content(&table_name, DateTimeAsMicroseconds::now())
        .await;

    crate::operations::sync::dispatch(
        app.as_ref(),
        &db_namespace,
        SyncEvent::DeleteTable(sync_data),
    );

    Ok(())
}

pub async fn init(
    _app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    db_table: DbTableInner,
) -> Arc<DbTable> {
    let db_table = DbTable::new(db_table);
    db_namespace.db.insert(db_table.clone());
    db_table
}

enum CreateTableResult {
    JustCreated(Arc<DbTable>),
    AlreadyHadTable(Arc<DbTable>),
}

async fn get_or_create_table(
    _app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    table_name: &str,
    persist: bool,
    max_partitions_amount: Option<usize>,
    max_rows_per_partition_amount: Option<usize>,
    compressed: bool,
    now: DateTimeAsMicroseconds,
) -> CreateTableResult {
    let (db_table, just_created) = db_namespace.db.get_or_create(table_name, || {
        let table_attributes = DbTableAttributes {
            persist,
            max_partitions_amount,
            created: now,
            max_rows_per_partition_amount,
            compressed,
        };
        DbTable::new(DbTableInner::new(table_name.into(), table_attributes))
    });

    if just_created {
        CreateTableResult::JustCreated(db_table)
    } else {
        CreateTableResult::AlreadyHadTable(db_table)
    }
}
