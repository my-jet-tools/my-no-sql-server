use my_no_sql_sdk::core::db::*;

use std::sync::Arc;

use crate::app::{AppContext, DbNamespace};

use super::{EntitiesInitReader, TableAttributeInitContract};

pub async fn init_tables(
    app: &AppContext,
    db_namespace: &Arc<DbNamespace>,
    tables: Vec<impl TableAttributeInitContract>,
    mut entities_reader: impl EntitiesInitReader,
    save_to_db: bool,
) {
    for table_init_contract in tables {
        let (table_name, attr) = table_init_contract.into();
        let mut db_table = DbTableInner::new(table_name, attr);

        let db_rows = entities_reader.get_entities(db_table.name.as_str()).await;

        if let Some(db_rows) = db_rows {
            //let by_partition =
            //    rust_extensions::grouped_data::group_to_btree_map(db_rows.into_iter(), |itm| {
            //        itm.get_partition_key().to_string()
            //    });

            for db_row in db_rows {
                //db_table.insert_or_replace_row(db_row, set_last_write_moment);
                //let mut db_partition = DbPartition::new(partition_key);
                //for db_row in entities {
                //    db_partition.insert_row(db_row);
                // }

                db_table.insert_or_replace_row(db_row, None);
            }
        }

        let db_table = crate::db_operations::write::table::init(app, db_namespace, db_table).await;

        if save_to_db {
            let table_snapshot = db_table.get_table_snapshot();

            println!("Migrating table: {}", db_table.name.as_str());
            db_namespace
                .repo
                .save_table_metadata(&db_table.name, &table_snapshot.attr)
                .await;

            crate::operations::persist::scripts::sync_table_snapshot(
                db_namespace,
                &db_table.name,
                table_snapshot,
            )
            .await;
        }
    }
}
