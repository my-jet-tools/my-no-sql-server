use my_http_server::macros::*;
use my_http_server::{HttpContext, HttpFailResult, HttpOkResult, HttpOutputAsStream};
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use std::sync::Arc;

use crate::app::AppContext;

/// How much of the archive is read from the disk and handed to the response at
/// a time.
const DOWNLOAD_CHUNK_SIZE: usize = 64 * 1024;

/// How many chunks the response may run ahead of the client. Bounded on purpose:
/// this is what keeps a download of a namespace worth gigabytes at a few hundred
/// kilobytes of memory.
const DOWNLOAD_CHUNKS_IN_FLIGHT: usize = 2;

/// Suffix of the archive built for a download.
///
/// Deliberately not ending in `.zip`: `get_list_of_files` keys off that
/// extension, so an archive being downloaded is neither listed as a snapshot,
/// nor eligible for collection by `MaxBackupsToKeep`.
const DOWNLOAD_FILE_SUFFIX: &str = ".download.tmp";

#[http_route(
    method: "GET",
    route: "/api/Backup/Download",
    description: "Download all tables as Zip Archive",
    summary: "Download all tables as Zip Archive",
    controller: "Backup",
    result:[
        {status_code: 200, description: "Snapshot of all tables"},
        {status_code: 500, description: "The snapshot could not be built"},
    ]
)]
pub struct DownloadAction {
    app: Arc<AppContext>,
}

impl DownloadAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

async fn handle_request(
    action: &DownloadAction,
    ctx: &mut HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let db_namespace = crate::http_server::get_request_namespace_existing(&action.app, ctx).await?;

    let now = DateTimeAsMicroseconds::now();
    let download_name = format!("{}.zip", &now.to_rfc3339().replace(":", "_")[..19]);

    // Built into a file and streamed out of it: the archive of a namespace worth
    // downloading weighs what the namespace weighs, and it used to be held in
    // memory whole, on top of the data it is an archive of.
    let file_name = crate::operations::backup::utils::compile_backup_file(
        &action.app,
        &db_namespace.name,
        format!("{}{}", download_name, DOWNLOAD_FILE_SUFFIX).as_str(),
    );

    crate::operations::write_db_snapshot_as_zip_file(&db_namespace, file_name.clone())
        .await
        .map_err(HttpFailResult::as_fatal_error)?;

    let (tx, rx) = futures::channel::mpsc::channel(DOWNLOAD_CHUNKS_IN_FLIGHT);

    tokio::spawn(stream_file_and_remove_it(
        file_name,
        my_http_server::HttpOutputProducer::new(tx),
    ));

    HttpOutputAsStream::new(rx)
        .with_header("Content-Type", "application/zip")
        .with_header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", download_name),
        )
        .get_result()
}

/// Feeds the archive to the response and takes the file with it — whether the
/// client read all of it, gave up half way, or the disk failed under us.
async fn stream_file_and_remove_it(
    file_name: String,
    producer: my_http_server::HttpOutputProducer,
) {
    if let Err(err) = stream_file(file_name.as_str(), producer).await {
        println!("Can not stream the archive {}. Err: {}", file_name, err);
    }

    if let Err(err) = tokio::fs::remove_file(file_name.as_str()).await {
        println!(
            "Can not remove the archive {} of a finished download. Err: {}",
            file_name, err
        );
    }
}

async fn stream_file(
    file_name: &str,
    mut producer: my_http_server::HttpOutputProducer,
) -> Result<(), String> {
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(file_name)
        .await
        .map_err(|err| format!("Can not open the archive. Err: {}", err))?;

    loop {
        let mut chunk = vec![0u8; DOWNLOAD_CHUNK_SIZE];

        let read = file
            .read(chunk.as_mut_slice())
            .await
            .map_err(|err| format!("Can not read the archive. Err: {}", err))?;

        if read == 0 {
            return Ok(());
        }

        chunk.truncate(read);

        // Fails once the client is gone, which is the moment to stop reading.
        producer
            .send(chunk)
            .await
            .map_err(|err| format!("The client is not reading the archive. Err: {}", err))?;
    }
}
