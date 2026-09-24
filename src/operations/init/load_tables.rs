use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use my_no_sql_sdk::core::db::DbTableAttributes;
use my_no_sql_sdk::server::rust_extensions::StopWatch;

use crate::app::{AppContext, DbNamespace};
use crate::persist_repo::LoadedTableAttrs;

use super::partitions_init_reader::PartitionsInitReader;

pub async fn load_tables(app: Arc<AppContext>) {
    let sw = StopWatch::new();

    if let Some(server_url) = app.settings.get_init_from_other_server_url() {
        load_from_other_instance(&app, server_url).await;
    } else {
        // Every namespace found on disk at start up is loaded — a reader of one
        // of them must not have to wait for somebody to touch it first.
        for db_namespace in app.namespaces.get_all() {
            load_namespace(&app, &db_namespace).await;
        }
    }

    println!("Tables are loaded in {:?}", sw.duration());

    app.states.set_initialized();
}

/// Pulls the content of ONE namespace from another server. Which one is named
/// by the connection string of `InitFromOtherServerUrl`
/// (`host=http://...;ns=alpha`); without an `ns` it is the default namespace.
async fn load_from_other_instance(app: &Arc<AppContext>, server_url: &str) {
    let connection_string =
        my_no_sql_sdk::parse_connection_string(server_url).unwrap_or_else(|err| {
            panic!(
                "Can not parse InitFromOtherServerUrl '{}'. Err: {}",
                server_url, err
            )
        });

    let db_namespace = app
        .namespaces
        .get_or_create(connection_string.get_namespace())
        .await;

    let server_url = connection_string.host.as_str();

    // The import below writes without the normal local-load having run;
    // the Files backend must scan its page-files first (see
    // PersistRepo::prime_for_writes) or the import would be silently
    // reverted on the next restart.
    db_namespace
        .repo
        .prime_for_writes(app.settings.skip_broken_partitions)
        .await;

    let tables = super::from_other_instance::load_tables(server_url).await;
    let tables_amount = tables.len();

    let entities_reader =
        super::from_other_instance::EntitiesInitReaderForOtherInstance::new(server_url.to_string());

    super::scripts::init_tables(app, &db_namespace, tables, entities_reader, true).await;

    println!(
        "Loaded {} table(s) of the namespace '{}' from {}",
        tables_amount, db_namespace.name, server_url
    );
}

async fn load_namespace(app: &Arc<AppContext>, db_namespace: &Arc<DbNamespace>) {
    let mut tables = db_namespace.repo.get_tables().await;
    let partitions = db_namespace
        .repo
        .load_all_partitions(app.settings.skip_broken_partitions)
        .await;

    // Data-safety net: a partition can only exist without a metadata record
    // if the metadata was lost (corruption / external edit) — table creation
    // persists attributes before partitions, and `delete_table` removes
    // partitions before metadata. Recreate such orphan tables with default
    // attributes so their rows are loaded, not silently dropped.
    let known: HashSet<&str> = tables.iter().map(|t| t.table_name.as_str()).collect();
    let mut orphans: BTreeMap<String, usize> = BTreeMap::new();
    for partition in &partitions {
        if !known.contains(partition.table_name.as_str()) {
            *orphans.entry(partition.table_name.clone()).or_default() += 1;
        }
    }
    drop(known);
    for (table_name, partitions_amount) in orphans {
        println!(
            "WARNING: Table '{}' of the namespace '{}' has {} persisted partition(s) but no metadata record. Recreating it with default attributes so its data is not lost.",
            table_name, db_namespace.name, partitions_amount
        );
        tables.push(LoadedTableAttrs {
            table_name: table_name.into(),
            attr: DbTableAttributes::create_default(),
        });
    }

    let tables_amount = tables.len();
    let entities_reader =
        PartitionsInitReader::new(partitions, app.settings.skip_broken_partitions);
    super::scripts::init_tables(app, db_namespace, tables, entities_reader, false).await;

    println!(
        "Loaded {} table(s) of the namespace '{}'",
        tables_amount, db_namespace.name
    );
}
