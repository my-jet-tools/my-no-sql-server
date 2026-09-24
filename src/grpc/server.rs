use crate::app::AppContext;
use crate::mynosqlserver_grpc::writer_server::WriterServer;
use anyhow::*;
use std::net::SocketAddr;
use std::sync::Arc;
use tonic::transport::Server;

#[derive(Clone)]
pub struct MyNoSqlServerWriterGrpcSerice {
    pub app: Arc<AppContext>,
}

impl MyNoSqlServerWriterGrpcSerice {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

pub async fn start(app: Arc<AppContext>, port: u16) -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let service = MyNoSqlServerWriterGrpcSerice::new(app);

    println!("Listening to {:?} as grpc endpoint", addr);
    Server::builder()
        .add_service(WriterServer::new(service))
        .serve(addr)
        .await
        .context("Server error")
}
