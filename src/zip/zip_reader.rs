use std::io::{BufReader, Cursor, Read, Seek};

/// How much of the archive is pulled from the file at a time — the inflater asks
/// for reads far smaller than a read worth making.
const FILE_READ_BUFFER_SIZE: usize = 256 * 1024;

/// Reads a snapshot archive out of anything that can be read and seeked — the
/// file in the backup folder, or a `Vec<u8>` for an archive that arrived as a
/// request body.
pub struct ZipReader<TReader: Read + Seek> {
    zip: zip::ZipArchive<TReader>,
}

impl ZipReader<BufReader<std::fs::File>> {
    /// Opens the archive in place: nothing but its central directory is read
    /// until an entry is asked for, so restoring costs the entry being restored
    /// instead of the whole snapshot on top of it.
    pub fn open_file(file_name: &str) -> Result<Self, std::io::Error> {
        let file = std::fs::File::open(file_name)?;

        Self::new(BufReader::with_capacity(FILE_READ_BUFFER_SIZE, file))
    }

    /// Opening is a file to open and a central directory to parse, both of them
    /// blocking, so it happens off the runtime worker.
    pub async fn open_file_on_blocking_thread(file_name: String) -> Result<Self, std::io::Error> {
        run_blocking(move || Self::open_file(file_name.as_str())).await
    }
}

impl ZipReader<Cursor<Vec<u8>>> {
    /// Only for an archive already held in memory — one uploaded to
    /// `RestoreFromZip`. A snapshot on disk goes through `open_file`.
    pub fn in_memory(zip_content: Vec<u8>) -> Result<Self, std::io::Error> {
        Self::new(Cursor::new(zip_content))
    }
}

impl<TReader: Read + Seek> ZipReader<TReader> {
    pub fn new(reader: TReader) -> Result<Self, std::io::Error> {
        // An archive can be an uploaded file, so a corrupt one has to be an
        // answer to give and not the panic this used to be.
        Ok(Self {
            zip: zip::ZipArchive::new(reader)?,
        })
    }

    pub fn get_file_names(&mut self) -> impl Iterator<Item = &str> {
        self.zip.file_names()
    }

    /// Inflates one entry. CPU and allocation on the size of the entry — callers
    /// are expected to be on a blocking thread.
    pub fn get_content_as_vec(&mut self, file_name: &str) -> Result<Vec<u8>, std::io::Error> {
        let mut file = self.zip.by_name(file_name)?;

        let declared_size = file.size() as usize;

        let mut content = Vec::new();

        // Asked for as one allocation, and refused rather than aborting the
        // process: the size is read out of the archive, which is not always ours.
        content.try_reserve_exact(declared_size).map_err(|_| {
            std::io::Error::other(format!(
                "Can not allocate {} bytes for the entry '{}' of the archive",
                declared_size, file_name
            ))
        })?;

        file.read_to_end(&mut content)?;

        Ok(content)
    }
}

impl<TReader: Read + Seek + Send + 'static> ZipReader<TReader> {
    /// Inflates one entry on a blocking thread and hands the reader back.
    ///
    /// The reader has to cross the thread boundary because the caller writes
    /// each entry into the database — an await — before asking for the next one.
    pub async fn read_entry(
        mut self,
        file_name: String,
    ) -> Result<(Self, Vec<u8>), std::io::Error> {
        run_blocking(move || {
            let content = self.get_content_as_vec(file_name.as_str())?;
            Ok((self, content))
        })
        .await
    }
}

async fn run_blocking<TResult: Send + 'static>(
    action: impl FnOnce() -> Result<TResult, std::io::Error> + Send + 'static,
) -> Result<TResult, std::io::Error> {
    tokio::task::spawn_blocking(action).await.map_err(|err| {
        std::io::Error::other(format!("The archive task did not finish. Err: {}", err))
    })?
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const ENTRY_NAME: &str = "test-table/pk";

    fn archive_with_entry(content: &[u8]) -> Vec<u8> {
        let mut zip_writer = zip::ZipWriter::new(Cursor::new(Vec::new()));

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        zip_writer.start_file(ENTRY_NAME, options).unwrap();
        zip_writer.write_all(content).unwrap();

        zip_writer.finish().unwrap().into_inner()
    }

    fn temp_file(content: &[u8]) -> String {
        let file_name = format!(
            "{}/zip_reader_{}.zip",
            std::env::temp_dir().display(),
            uuid::Uuid::new_v4()
        );

        std::fs::write(file_name.as_str(), content).unwrap();

        file_name
    }

    #[test]
    fn test_an_entry_read_from_the_file_is_the_entry_written_into_it() {
        // Deliberately several read buffers long and not a multiple of one: the
        // archive is pulled through that buffer now instead of being read whole,
        // and the entry is filled by a loop that has to stop on its declared
        // size and not on a buffer boundary.
        let content: Vec<u8> = (0..FILE_READ_BUFFER_SIZE * 3 + 7)
            .map(|index| (index % 251) as u8)
            .collect();

        let file_name = temp_file(archive_with_entry(content.as_slice()).as_slice());

        let mut zip_reader = ZipReader::open_file(file_name.as_str()).unwrap();

        assert_eq!(content, zip_reader.get_content_as_vec(ENTRY_NAME).unwrap());

        std::fs::remove_file(file_name).ok();
    }

    #[test]
    fn test_the_file_reader_and_the_memory_reader_agree() {
        let archive = archive_with_entry(b"[{\"PartitionKey\":\"pk\"}]");
        let file_name = temp_file(archive.as_slice());

        let from_file = ZipReader::open_file(file_name.as_str())
            .unwrap()
            .get_content_as_vec(ENTRY_NAME)
            .unwrap();

        let from_memory = ZipReader::in_memory(archive)
            .unwrap()
            .get_content_as_vec(ENTRY_NAME)
            .unwrap();

        assert_eq!(from_file, from_memory);

        std::fs::remove_file(file_name).ok();
    }

    #[test]
    fn test_a_content_which_is_not_an_archive_is_an_error() {
        // Reachable with an uploaded file — it used to take the server down.
        assert!(ZipReader::in_memory(b"this is not an archive".to_vec()).is_err());
    }

    #[test]
    fn test_a_truncated_archive_is_an_error() {
        let mut archive = archive_with_entry(b"a partition of a table");
        archive.truncate(archive.len() / 2);

        assert!(ZipReader::in_memory(archive).is_err());
    }

    #[test]
    fn test_an_entry_which_is_not_in_the_archive_is_an_error() {
        let mut zip_reader = ZipReader::in_memory(archive_with_entry(b"[]")).unwrap();

        assert!(zip_reader.get_content_as_vec("test-table/other-pk").is_err());
    }
}
