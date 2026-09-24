use std::sync::Arc;

use my_no_sql_sdk::core::db::DbRow;
use my_no_sql_sdk::core::my_json::json_writer::JsonArrayWriter;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;

pub fn filter_and_compile_json<'s>(
    iterator: impl Iterator<Item = &'s Arc<DbRow>>,
    limit: Option<usize>,
    skip: Option<usize>,
    handle: impl Fn(&'s Arc<DbRow>),
) -> JsonArrayWriter {
    let mut result = JsonArrayWriter::new();

    let mut no = 0;
    let mut added = 0;

    for db_row in iterator {
        if let Some(skip) = skip {
            if no < skip {
                no += 1;
                continue;
            }
        }

        handle(db_row);
        result = result.write(db_row.as_ref());
        added += 1;

        if let Some(limit) = limit {
            if added >= limit {
                break;
            }
        }

        no += 1;
        //json_array_writer.write_raw_element(&db_row.data);
        //crate::db_operations::sync_to_main::update_row_last_read_access_time(app, db_row);
    }

    result
    //json_array_writer.build()
}

pub fn filter_it<'s, TItem>(
    iterator: impl Iterator<Item = &'s TItem>,
    limit: Option<usize>,
    skip: Option<usize>,
) -> Vec<&'s TItem> {
    let mut result = if let Some(limit) = limit {
        Vec::with_capacity(limit)
    } else {
        Vec::new()
    };

    let mut no = 0;
    let mut added = 0;

    for item in iterator {
        if let Some(skip) = skip {
            if no < skip {
                no += 1;
                continue;
            }
        }

        result.push(item);
        added += 1;

        if let Some(limit) = limit {
            if added >= limit {
                break;
            }
        }

        no += 1;
        //json_array_writer.write_raw_element(&db_row.data);
        //crate::db_operations::sync_to_main::update_row_last_read_access_time(app, db_row);
    }

    result
    //json_array_writer.build()
}

pub fn filter_it_and_clone<'s, TIter: Iterator<Item = &'s Arc<DbRow>>>(
    iterator: TIter,
    limit: Option<usize>,
    skip: Option<usize>,
    now: DateTimeAsMicroseconds,
) -> Vec<Arc<DbRow>> {
    let mut result = if let Some(limit) = limit {
        Vec::with_capacity(limit)
    } else {
        Vec::new()
    };

    let mut no = 0;
    let mut added = 0;

    for db_row in iterator {
        if let Some(skip) = skip {
            if no < skip {
                no += 1;
                continue;
            }
        }
        db_row.update_last_read_access(now);
        result.push(db_row.clone());
        added += 1;

        if let Some(limit) = limit {
            if added >= limit {
                break;
            }
        }

        no += 1;
        //json_array_writer.write_raw_element(&db_row.data);
        //crate::db_operations::sync_to_main::update_row_last_read_access_time(app, db_row);
    }

    result
    //json_array_writer.build()
}
