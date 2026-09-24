use app::AppContext;
use background::{
    gc_bulk_processes::GcBulkProcesses, gc_db_rows::GcDbRows,
    gc_http_sessions::GcHttpSessionsTimer, gc_multipart::GcMultipart,
    metrics_updater::MetricsUpdater, persist::PersistTimer, sync::SyncEventLoop, BackupTimer,
    GcBackupsTimer, VacuumTimer,
};

use my_no_sql_sdk::core::rust_extensions::MyTimer;
use my_no_sql_sdk::tcp_contracts::MyNoSqlTcpSerializerFactory;
use my_tcp_sockets::TcpServer;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tcp::TcpServerEvents;
mod zip;

mod app;
mod consts;
mod files_repo;
mod grpc;
mod persist_compression;
mod persist_markers;
mod persist_repo;

mod db_operations;
mod db_sync;
mod db_transactions;
mod http_server;
mod scripts;
mod tcp;

mod background;
mod data_readers;
mod mcp;
mod operations;
mod settings_reader;

pub mod mynosqlserver_grpc {
    tonic::include_proto!("mynosqlserver");
}

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    let settings = settings_reader::read_settings().await;

    let settings = Arc::new(settings);

    let app = AppContext::new(settings).await;

    let app = Arc::new(app);

    app.migrate_backup_folder().await;

    tokio::spawn(crate::operations::init::load_tables(app.clone()));

    let http_connections_counter = crate::http_server::start_up::setup_server(&app).await;

    app.sync
        .register_event_loop(Arc::new(SyncEventLoop::new(app.clone())));

    let reader_tcp_addr = SocketAddr::from(([0, 0, 0, 0], 5125));

    println!("Listening reader at TCP addr: '{}'", reader_tcp_addr);

    let tcp_server = TcpServer::new("MyNoSqlReaderTcp".to_string(), reader_tcp_addr);

    let unix_reader = if let Some(unix_socket) = app.use_unix_socket.clone() {
        let mut file_path = unix_socket.clone();
        file_path.append_segment(crate::consts::READER_UNIX_SOCKET_NAME);

        println!(
            "Listening reader at unix-socket addr: '{}'",
            file_path.as_str()
        );

        let unix_reader = my_tcp_sockets::unix_socket_server::UnixSocketServer::new(
            "MyNoSqlReaderUnixSocket".to_string(),
            file_path.into_string(),
        );

        Some(unix_reader)
    } else {
        None
    };

    let mut timer_1s = MyTimer::new(Duration::from_secs(1));

    let mut persist_timer = MyTimer::new(Duration::from_secs(1));

    persist_timer.register_timer("Persist", Arc::new(PersistTimer::new(app.clone())));

    timer_1s.register_timer(
        "MetricsUpdated",
        Arc::new(MetricsUpdater::new(
            app.clone(),
            http_connections_counter,
            tcp_server.threads_statistics.clone(),
            unix_reader
                .as_ref()
                .map(|itm| itm.threads_statistics.clone()),
        )),
    );

    let mut timer_10s = MyTimer::new(Duration::from_secs(10));
    timer_10s.register_timer(
        "GcHttpSessions",
        Arc::new(GcHttpSessionsTimer::new(app.clone())),
    );

    let mut timer_30s = MyTimer::new(Duration::from_secs(30));
    timer_30s.register_timer("GcDbRows", Arc::new(GcDbRows::new(app.clone())));
    timer_30s.register_timer("GcMultipart", Arc::new(GcMultipart::new(app.clone())));
    timer_30s.register_timer(
        "GcBulkProcesses",
        Arc::new(GcBulkProcesses::new(app.clone())),
    );

    timer_1s.start(app.states.clone(), my_logger::LOGGER.clone());
    timer_10s.start(app.states.clone(), my_logger::LOGGER.clone());
    timer_30s.start(app.states.clone(), my_logger::LOGGER.clone());
    persist_timer.start(app.states.clone(), my_logger::LOGGER.clone());

    let mut backup_timer = MyTimer::new(Duration::from_secs(60));

    backup_timer.register_timer("BackupDb", Arc::new(BackupTimer::new(app.clone())));
    backup_timer.register_timer("GcBackups", Arc::new(GcBackupsTimer::new(app.clone())));
    backup_timer.register_timer("Vacuum", Arc::new(VacuumTimer::new(app.clone())));

    backup_timer.start(app.states.clone(), my_logger::LOGGER.clone());

    app.sync
        .start(app.states.clone(), my_logger::LOGGER.clone());

    if let Some(unix_reader) = unix_reader.as_ref() {
        unix_reader
            .start(
                Arc::new(MyNoSqlTcpSerializerFactory),
                TcpServerEvents::new(app.clone()),
                app.states.clone(),
                my_logger::LOGGER.clone(),
            )
            .await;
    }

    tcp_server
        .start(
            Arc::new(MyNoSqlTcpSerializerFactory),
            TcpServerEvents::new(app.clone()),
            app.states.clone(),
            my_logger::LOGGER.clone(),
        )
        .await;

    tokio::task::spawn(crate::grpc::server::start(app.clone(), 5124));

    app.states.wait_until_shutdown().await;

    crate::operations::shutdown(&app).await;
}
