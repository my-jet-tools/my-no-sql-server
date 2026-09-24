use std::io::{Seek, Write};

use my_no_sql_sdk::server::db_snapshots::{DbRowsSnapshot, DbTableSnapshot};

/// How much serialized JSON is allowed to pile up before it is handed to the
/// archive.
///
/// Rows are appended to one reused buffer and flushed as it fills, instead of
/// the partition being turned into a single JSON string first. That string was
/// the uncompressed size of the whole partition, held on top of the archive
/// itself, and for the tables worth backing up it was the larger of the two.
const JSON_FLUSH_THRESHOLD: usize = 64 * 1024;

/// Writes a snapshot of a database as a zip archive into any sink that can be
/// written and seeked — a file for the backup on disk, a `Vec<u8>` for the
/// download endpoint that has to answer with a body.
pub struct DbZipBuilder<TWriter: Write + Seek> {
    zip_writer: zip::ZipWriter<TWriter>,
    /// Reused across every partition of every table: the archive costs one
    /// buffer of `JSON_FLUSH_THRESHOLD` plus the largest single row, not one
    /// buffer per partition.
    json_buffer: String,
}

impl<TWriter: Write + Seek> DbZipBuilder<TWriter> {
    pub fn new(writer: TWriter) -> Self {
        Self {
            zip_writer: zip::ZipWriter::new(writer),
            json_buffer: String::with_capacity(JSON_FLUSH_THRESHOLD),
        }
    }

    pub fn add_table(
        &mut self,
        table_name: &str,
        content: &DbTableSnapshot,
    ) -> Result<(), zip::result::ZipError> {
        let file_name = format!(
            "{}/{}",
            table_name,
            crate::scripts::TABLE_METADATA_FILE_NAME
        );

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        self.zip_writer.start_file(file_name, options)?;

        let payload = crate::scripts::serializers::table_attrs::serialize(&content.attr);
        write_to_zip_file(&mut self.zip_writer, &payload)?;

        for itm in &content.by_partition {
            use base64::Engine;
            let encoded_file_name = base64::engine::general_purpose::STANDARD
                .encode(itm.partition_key.as_str().as_bytes());
            let file_name = format!("{}/{}", table_name, encoded_file_name);

            self.zip_writer.start_file(file_name, options)?;

            self.write_partition_rows(&itm.db_rows_snapshot)?;
        }

        Ok(())
    }

    /// The partition as a JSON array of its rows — byte for byte what
    /// `DbRowsSnapshot::as_json_array().build()` produces, written in pieces
    /// instead of assembled whole.
    fn write_partition_rows(
        &mut self,
        db_rows_snapshot: &DbRowsSnapshot,
    ) -> Result<(), zip::result::ZipError> {
        self.json_buffer.clear();
        self.json_buffer.push('[');

        for (index, db_row) in db_rows_snapshot.db_rows.iter().enumerate() {
            if index > 0 {
                self.json_buffer.push(',');
            }

            db_row.write_json(&mut self.json_buffer);

            if self.json_buffer.len() >= JSON_FLUSH_THRESHOLD {
                flush_json_buffer(&mut self.zip_writer, &mut self.json_buffer)?;
            }
        }

        self.json_buffer.push(']');

        flush_json_buffer(&mut self.zip_writer, &mut self.json_buffer)
    }

    /// Closes the archive and gives the sink back — for a file that is what the
    /// caller has to flush and sync before renaming it into place.
    pub fn finish(self) -> Result<TWriter, zip::result::ZipError> {
        self.zip_writer.finish()
    }
}

/// Hands what has piled up to the archive and empties the buffer, keeping its
/// capacity so the next rows do not allocate again.
fn flush_json_buffer<TWriter: Write + Seek>(
    zip_writer: &mut zip::ZipWriter<TWriter>,
    json_buffer: &mut String,
) -> Result<(), zip::result::ZipError> {
    write_to_zip_file(zip_writer, json_buffer.as_bytes())?;
    json_buffer.clear();

    Ok(())
}

fn write_to_zip_file<TWriter: Write + Seek>(
    zip_writer: &mut zip::ZipWriter<TWriter>,
    payload: &[u8],
) -> Result<(), zip::result::ZipError> {
    let mut pos = 0;
    while pos < payload.len() {
        let size = zip_writer.write(&payload[pos..])?;

        pos += size;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use my_no_sql_sdk::core::db::{DbRow, DbTableAttributes, PartitionKey};
    use my_no_sql_sdk::core::db_json_entity::DbJsonEntity;
    use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
    use my_no_sql_sdk::server::db_snapshots::{
        DbPartitionSnapshot, DbRowsSnapshot, DbTableSnapshot,
    };

    use super::*;

    const TABLE_NAME: &str = "test-table";
    const PARTITION_KEY: &str = "pk";

    /// A row as it comes out of a backup file — `TimeStamp` included, which is
    /// what a stored row always carries.
    fn db_row(row_key: &str, payload_size: usize) -> Arc<DbRow> {
        let raw = format!(
            r#"{{"PartitionKey":"{}","RowKey":"{}","TimeStamp":"2026-08-21T12:00:00","Payload":"{}"}}"#,
            PARTITION_KEY,
            row_key,
            "x".repeat(payload_size)
        );

        Arc::new(DbJsonEntity::restore_into_db_row(raw.into_bytes()).unwrap())
    }

    fn table_snapshot(db_rows: Vec<Arc<DbRow>>) -> DbTableSnapshot {
        DbTableSnapshot {
            attr: DbTableAttributes::create_default(),
            last_write_moment: DateTimeAsMicroseconds::new(0),
            by_partition: vec![DbPartitionSnapshot {
                last_read_moment: DateTimeAsMicroseconds::new(0),
                last_write_moment: DateTimeAsMicroseconds::new(0),
                partition_key: PartitionKey::new(PARTITION_KEY.to_string()),
                db_rows_snapshot: DbRowsSnapshot::new_from_snapshot(db_rows),
            }],
        }
    }

    /// The partition as it lands in the archive.
    fn partition_content_of_the_archive(table_snapshot: &DbTableSnapshot) -> String {
        let file_name = format!(
            "{}/db_zip_builder_{}.zip",
            std::env::temp_dir().display(),
            uuid::Uuid::new_v4()
        );

        let file = std::fs::File::create(file_name.as_str()).unwrap();

        let mut zip_builder = DbZipBuilder::new(std::io::BufWriter::new(file));
        zip_builder.add_table(TABLE_NAME, table_snapshot).unwrap();
        zip_builder.finish().unwrap().into_inner().unwrap();

        let mut zip_reader = crate::zip::ZipReader::open_file(file_name.as_str()).unwrap();

        use base64::Engine;
        let entry_name = format!(
            "{}/{}",
            TABLE_NAME,
            base64::engine::general_purpose::STANDARD.encode(PARTITION_KEY.as_bytes())
        );

        let content = zip_reader.get_content_as_vec(entry_name.as_str()).unwrap();

        std::fs::remove_file(file_name).ok();

        String::from_utf8(content).unwrap()
    }

    #[test]
    fn test_a_partition_spanning_many_flushes_is_the_same_json() {
        // The rewrite this guards: the partition used to be assembled into one
        // JSON string and handed over whole. Now it is flushed in pieces, and a
        // piece boundary must not be visible in the result — this partition
        // crosses the threshold a few times over.
        let rows_per_flush = 4;
        let payload_size = JSON_FLUSH_THRESHOLD / rows_per_flush;

        let db_rows: Vec<Arc<DbRow>> = (0..rows_per_flush * 3)
            .map(|index| db_row(format!("row-{}", index).as_str(), payload_size))
            .collect();

        let table_snapshot = table_snapshot(db_rows);

        let expected = table_snapshot.by_partition[0]
            .db_rows_snapshot
            .as_json_array()
            .build();

        assert!(expected.len() > JSON_FLUSH_THRESHOLD * 2);
        assert_eq!(expected, partition_content_of_the_archive(&table_snapshot));
    }

    #[test]
    fn test_a_partition_shorter_than_a_flush_is_the_same_json() {
        let table_snapshot = table_snapshot(vec![db_row("row-0", 8), db_row("row-1", 8)]);

        let expected = table_snapshot.by_partition[0]
            .db_rows_snapshot
            .as_json_array()
            .build();

        assert_eq!(expected, partition_content_of_the_archive(&table_snapshot));
    }

    #[test]
    fn test_a_partition_with_no_rows_is_an_empty_json_array() {
        // Nothing is written between the brackets, and the entry still has to be
        // valid JSON — the restore path parses it as an array.
        let table_snapshot = table_snapshot(Vec::new());

        assert_eq!("[]", partition_content_of_the_archive(&table_snapshot));
    }
}
