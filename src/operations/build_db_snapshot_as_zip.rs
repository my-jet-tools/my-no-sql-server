use std::io::BufWriter;
use std::sync::Arc;

use crate::{app::DbNamespace, zip::DbZipBuilder};

/// Suffix of the file a snapshot is written into before it is published under
/// its real name.
///
/// The publishing step is a rename, which is atomic: whoever lists the folder
/// sees either no snapshot or a complete one, never a growing prefix of one.
/// Deliberately not ending in `.zip` — `get_list_of_files` keys off that
/// extension, so a snapshot in flight is neither listed, nor downloadable, nor
/// eligible for collection by `MaxBackupsToKeep`.
const TEMP_FILE_SUFFIX: &str = ".tmp";

/// The archive is handed over in whatever pieces the compressor emits, which are
/// far smaller than a write worth making — this is what turns them into one.
const FILE_WRITE_BUFFER_SIZE: usize = 256 * 1024;

/// Zips a snapshot of every table of the namespace straight into `file_name`.
///
/// Nothing but one flush buffer is held in memory: a namespace whose tables
/// weigh gigabytes used to be held a second time as the finished archive, and
/// a third time transiently whenever that buffer had to grow — which is what
/// made the backup tick, not the traffic, the peak of the process.
///
/// The whole archive is built on a blocking thread. Deflating gigabytes is
/// seconds to minutes of uninterrupted CPU, and it used to run on a runtime
/// worker, so every request that landed on that worker waited out the backup.
pub async fn write_db_snapshot_as_zip_file(
    db_namespace: &Arc<DbNamespace>,
    file_name: String,
) -> Result<(), String> {
    let db_namespace = db_namespace.clone();

    tokio::task::spawn_blocking(move || write_snapshot(&db_namespace, file_name.as_str()))
        .await
        .map_err(|err| format!("The snapshot task did not finish. Err: {}", err))?
}

fn write_snapshot(db_namespace: &Arc<DbNamespace>, file_name: &str) -> Result<(), String> {
    publish_through_temp_file(file_name, |temp_file_name| {
        build_archive_file(db_namespace, temp_file_name)
    })
}

/// Runs `write_content` against a temporary path and publishes the result under
/// `file_name`, creating the folder when it is missing.
///
/// A failure leaves neither file: the temporary one is removed, so a backup
/// folder does not accumulate the leftovers of runs that did not go through. The
/// temporary name carries the same time stamp as the snapshot it would have
/// become, so it is never reused — the one leftover nothing cleans up is the
/// process dying mid-archive, and it names the run it belongs to.
fn publish_through_temp_file(
    file_name: &str,
    write_content: impl FnOnce(&str) -> Result<(), String>,
) -> Result<(), String> {
    if let Some(folder) = std::path::Path::new(file_name).parent() {
        std::fs::create_dir_all(folder).map_err(|err| {
            format!(
                "Can not create the backup folder {}. Err: {}",
                folder.display(),
                err
            )
        })?;
    }

    let temp_file_name = format!("{}{}", file_name, TEMP_FILE_SUFFIX);

    let result = write_content(temp_file_name.as_str()).and_then(|_| {
        std::fs::rename(temp_file_name.as_str(), file_name).map_err(|err| {
            format!(
                "Can not publish the snapshot as {}. Err: {}",
                file_name, err
            )
        })
    });

    if result.is_err() {
        std::fs::remove_file(temp_file_name.as_str()).ok();
    }

    result
}

fn build_archive_file(db_namespace: &Arc<DbNamespace>, file_name: &str) -> Result<(), String> {
    let file = std::fs::File::create(file_name)
        .map_err(|err| format!("Can not create {}. Err: {}", file_name, err))?;

    let mut zip_builder = DbZipBuilder::new(BufWriter::with_capacity(FILE_WRITE_BUFFER_SIZE, file));

    for db_table in db_namespace.db.get_tables().iter() {
        // One table at a time: the snapshot is a vector of row handles, so it
        // is cheap, but it does pin every row it holds against the garbage
        // collector until the table is in the archive.
        let table_snapshot = db_table.get_table_snapshot();

        zip_builder
            .add_table(db_table.name.as_str(), &table_snapshot)
            .map_err(|err| {
                format!(
                    "Can not add the table {} to the archive. Err: {}",
                    db_table.name, err
                )
            })?;
    }

    let file = zip_builder
        .finish()
        .map_err(|err| format!("Can not compile the archive. Err: {}", err))?;

    let file = file
        .into_inner()
        .map_err(|err| format!("Can not flush the archive. Err: {}", err))?;

    // The snapshot is the copy that has to survive the machine it is written on,
    // so it is not left in the page cache: a crash between the write and the
    // flush would leave a file of the right size holding holes.
    file.sync_all()
        .map_err(|err| format!("Can not flush the archive to the disk. Err: {}", err))
}

#[cfg(test)]
mod tests {
    fn temp_folder() -> String {
        format!(
            "{}/snapshot_{}",
            std::env::temp_dir().display(),
            uuid::Uuid::new_v4()
        )
    }

    #[test]
    fn test_the_snapshot_is_published_under_its_name_and_leaves_no_temp_file() {
        // The backup folder of a namespace which has never been backed up does
        // not exist yet, so publishing has to create it.
        let folder = temp_folder();
        let file_name = format!("{}/20260821T120000.zip", folder);

        let result = super::publish_through_temp_file(file_name.as_str(), |temp_file_name| {
            std::fs::write(temp_file_name, b"archive").map_err(|err| err.to_string())
        });

        assert_eq!(Ok(()), result);
        assert_eq!(
            b"archive".to_vec(),
            std::fs::read(file_name.as_str()).unwrap()
        );
        assert_eq!(
            false,
            std::path::Path::new(format!("{}{}", file_name, super::TEMP_FILE_SUFFIX).as_str())
                .exists()
        );

        std::fs::remove_dir_all(folder).ok();
    }

    #[test]
    fn test_a_snapshot_which_did_not_go_through_leaves_nothing_behind() {
        // What the folder must never hold: a partial archive under a name that
        // looks like a finished snapshot, or a temporary file nothing collects.
        let folder = temp_folder();
        let file_name = format!("{}/20260821T120000.zip", folder);

        let result = super::publish_through_temp_file(file_name.as_str(), |temp_file_name| {
            std::fs::write(temp_file_name, b"half an archive").unwrap();
            Err("the disk filled up".to_string())
        });

        assert_eq!(Err("the disk filled up".to_string()), result);
        assert_eq!(false, std::path::Path::new(file_name.as_str()).exists());
        assert_eq!(
            false,
            std::path::Path::new(format!("{}{}", file_name, super::TEMP_FILE_SUFFIX).as_str())
                .exists()
        );

        std::fs::remove_dir_all(folder).ok();
    }
}
