use std::{net::SocketAddr, sync::Arc};

use mcp_server_middleware::McpMiddleware;
use my_http_server::controllers::swagger::SwaggerMiddleware;
use my_http_server::{HttpConnectionsCounter, MyHttpServer};

use crate::app::AppContext;

pub async fn setup_server(app: &Arc<AppContext>) -> HttpConnectionsCounter {
    let http_port = SocketAddr::from(([0, 0, 0, 0], 5123));
    println!("Starting HTTP server at Tcp({:?})", http_port);
    let mut http_server = MyHttpServer::new(http_port);

    let mut unix_socket_http_server = if let Some(mut unix_socket) = app.use_unix_socket.clone() {
        unix_socket.append_segment(crate::consts::WRITER_UNIX_SOCKET_NAME);

        println!("Starting HTTP server at Unix({:?})", unix_socket.as_str());
        let http_server = MyHttpServer::new_as_unix_socket(unix_socket.into_string());
        Some(http_server)
    } else {
        None
    };

    let controllers = Arc::new(crate::http_server::controllers::builder::build(app));

    let swagger_middleware = SwaggerMiddleware::new(
        controllers.clone(),
        "MyNoSqlServer".to_string(),
        crate::app::APP_VERSION.to_string(),
    );
    let swagger_middleware = Arc::new(swagger_middleware);

    let static_files_middleware = Arc::new(
        my_http_server::StaticFilesMiddleware::new()
            .add_index_file("index.html")
            .set_not_found_file("index.html".to_string()),
    );

    let mcp_middleware = Arc::new(build_mcp_middleware(app).await);

    let statistics_middleware =
        Arc::new(crate::http_server::StatisticsMiddleware::new(app.clone()));

    if let Some(unix_socket_http_server) = unix_socket_http_server.as_mut() {
        unix_socket_http_server.add_middleware(statistics_middleware.clone());
        unix_socket_http_server.add_middleware(swagger_middleware.clone());
        unix_socket_http_server.add_middleware(controllers.clone());
        unix_socket_http_server.add_middleware(mcp_middleware.clone());
        unix_socket_http_server.add_middleware(static_files_middleware.clone());
    }

    http_server.add_middleware(statistics_middleware);
    http_server.add_middleware(swagger_middleware);
    http_server.add_middleware(controllers);
    http_server.add_middleware(mcp_middleware);
    http_server.add_middleware(static_files_middleware);

    // One TCP listener serving both protocols: hyper's auto builder sniffs the HTTP/2 preface, so
    // h1 and h2c (prior-knowledge) clients are accepted on the same port.
    http_server.start_auto(app.states.clone(), my_logger::LOGGER.clone());

    if let Some(unix_socket_http_server) = unix_socket_http_server.as_mut() {
        unix_socket_http_server.start(app.states.clone(), my_logger::LOGGER.clone());
    }

    http_server.get_http_connections_counter()
}

async fn build_mcp_middleware(app: &Arc<AppContext>) -> McpMiddleware {
    let mut mcp = McpMiddleware::new(
        "/mcp",
        crate::app::APP_NAME,
        crate::app::APP_VERSION,
        "Provides access to MyNoSqlServer data",
    );

    mcp.register_tool_call(Arc::new(crate::mcp::GetRowsToolCallHandler::new(
        app.clone(),
    )));

    mcp.register_tool_call(Arc::new(crate::mcp::GetListOfTablesToolCallHandler::new(
        app.clone(),
    )));

    mcp.register_tool_call(Arc::new(crate::mcp::GetTableClientsToolCallHandler::new(
        app.clone(),
    )));

    mcp.register_tool_call(Arc::new(crate::mcp::DeleteRowToolCallHandler::new(
        app.clone(),
    )));

    mcp.register_tool_call(Arc::new(crate::mcp::BulkDeleteRowsToolCallHandler::new(
        app.clone(),
    )));

    mcp.register_tool_call(Arc::new(
        crate::mcp::InsertOrReplaceRowToolCallHandler::new(app.clone()),
    ));

    mcp.register_tool_call(Arc::new(
        crate::mcp::BulkInsertOrReplaceRowsToolCallHandler::new(app.clone()),
    ));

    mcp.register_tool_call(Arc::new(crate::mcp::CleanTableToolCallHandler::new(
        app.clone(),
    )));

    mcp.register_tool_call(Arc::new(
        crate::mcp::MoveTableToNamespaceToolCallHandler::new(app.clone()),
    ));

    mcp.register_tool_call(Arc::new(crate::mcp::DeletePartitionsToolCallHandler::new(
        app.clone(),
    )));

    mcp.register_tool_call(Arc::new(crate::mcp::GetListOfBackupsToolCallHandler::new(
        app.clone(),
    )));

    mcp.register_tool_call(Arc::new(crate::mcp::GetBackupTablesToolCallHandler::new(
        app.clone(),
    )));

    mcp.register_tool_call(Arc::new(
        crate::mcp::GetBackupPartitionsToolCallHandler::new(app.clone()),
    ));

    mcp.register_tool_call(Arc::new(crate::mcp::GetBackupRowsToolCallHandler::new(
        app.clone(),
    )));

    mcp.register_tool_call(Arc::new(crate::mcp::RestoreBackupToolCallHandler::new(
        app.clone(),
    )));

    mcp.register_prompt(Arc::new(crate::mcp::McpWritesEnablePolicyPromptHandler));
    mcp.register_prompt(Arc::new(crate::mcp::PasteDeleteViaUiPromptHandler));

    mcp
}
