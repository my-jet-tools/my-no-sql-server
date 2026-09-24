use std::sync::Arc;

use my_logger::LogEventCtx;
use my_no_sql_sdk::tcp_contracts::{MyNoSqlReaderTcpSerializer, MyNoSqlTcpContract};
use my_tcp_sockets::{tcp_connection::TcpSocketConnection, SocketEventCallback};

use crate::{
    app::{AppContext, DbNamespace},
    data_readers::{tcp_connection::ReaderName, DataReader},
};

pub type MyNoSqlTcpConnection =
    TcpSocketConnection<MyNoSqlTcpContract, MyNoSqlReaderTcpSerializer, ()>;

#[derive(Clone)]
pub struct TcpServerEvents {
    app: Arc<AppContext>,
}

impl TcpServerEvents {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }

    /// Namespace the connection works in, as long as it exists. A connection
    /// which never sent `SetNamespace` stays in the default one, which always
    /// does.
    ///
    /// Resolved WITHOUT creating: every packet a TCP connection can send reads,
    /// and a mistyped `SetNamespace` must not leave a namespace — and a
    /// `db/<ns>` folder — behind. `None` costs a reader nothing it was not
    /// getting already: the namespace which used to be conjured up for it held
    /// no tables either, so the answer was "table not found" either way.
    fn get_existing_namespace(&self, data_reader: &Arc<DataReader>) -> Option<Arc<DbNamespace>> {
        self.app
            .namespaces
            .get(data_reader.get_namespace().as_str())
    }

    /// Namespace a `Subscribe` works in. Unlike everything else on this
    /// connection a subscribe MAY write — it creates the table it subscribes to
    /// when `AutoCreateTableOnReaderSubscribe` is on — and a packet allowed to
    /// create a table has to be allowed to create the namespace holding it.
    async fn get_namespace_of_subscribe(
        &self,
        data_reader: &Arc<DataReader>,
    ) -> Option<Arc<DbNamespace>> {
        if self.app.settings.auto_create_table_on_reader_subscribe {
            let namespace = data_reader.get_namespace();

            return Some(self.app.namespaces.get_or_create(namespace.as_str()).await);
        }

        self.get_existing_namespace(data_reader)
    }

    async fn set_namespace(
        &self,
        data_reader: &Arc<DataReader>,
        namespace: &str,
    ) -> Result<(), String> {
        if let Err(err) = my_no_sql_sdk::validate_namespace_name(namespace) {
            return Err(format!("Invalid namespace name. {}", err));
        }

        data_reader.set_namespace(namespace.into()).await?;

        // Deliberately NOT materialized here: naming a namespace is not writing
        // to it, and a reader which connects before the first writer must not be
        // the one that brings the namespace into existence. Whatever the reader
        // does next resolves it — by then it either exists or the reader is
        // answered that it does not.
        Ok(())
    }

    /// Namespace and table addressed by one of the maintenance packets, resolved
    /// inside the namespace of the connection which sent it. `None` as soon as
    /// any of the three is not there — which is what the packets treat as
    /// "nothing to update".
    async fn get_table_of_connection(
        &self,
        connection: &Arc<MyNoSqlTcpConnection>,
        table_name: &str,
    ) -> Option<(Arc<DbNamespace>, Arc<my_no_sql_sdk::server::DbTable>)> {
        let data_reader = self.app.data_readers.get_tcp(connection.as_ref()).await?;
        let db_namespace = self.get_existing_namespace(&data_reader)?;
        let db_table = db_namespace.db.get_table(table_name)?;

        Some((db_namespace, db_table))
    }
}

#[async_trait::async_trait]
impl SocketEventCallback<MyNoSqlTcpContract, MyNoSqlReaderTcpSerializer, ()> for TcpServerEvents {
    async fn connected(
        &mut self,
        _connection: Arc<TcpSocketConnection<MyNoSqlTcpContract, MyNoSqlReaderTcpSerializer, ()>>,
    ) {
        //println!("New connection");
        self.app.metrics.mark_new_tcp_connection();
    }

    async fn disconnected(
        &mut self,
        connection: Arc<TcpSocketConnection<MyNoSqlTcpContract, MyNoSqlReaderTcpSerializer, ()>>,
    ) {
        let name =
            if let Some(data_reader) = self.app.data_readers.get_tcp(connection.as_ref()).await {
                data_reader.get_name().to_string()
            } else {
                "".to_string()
            };

        my_logger::LOGGER.write_info(
            "TcpConnection",
            "Disconnected",
            LogEventCtx::new()
                .add("id", connection.id.to_string())
                .add("Name", name),
        );
        if let Some(data_reader) = self.app.data_readers.remove_tcp(connection.as_ref()).await {
            self.app
                .metrics
                .remove_pending_to_sync(&data_reader.connection);
        }
        self.app.metrics.mark_new_tcp_disconnection();
    }

    async fn payload(
        &mut self,
        connection: &Arc<TcpSocketConnection<MyNoSqlTcpContract, MyNoSqlReaderTcpSerializer, ()>>,
        contract: MyNoSqlTcpContract,
    ) {
        match contract {
            MyNoSqlTcpContract::Ping => {
                connection.send(&MyNoSqlTcpContract::Pong);
            }
            MyNoSqlTcpContract::PingWithLatency { micros } => {
                // A ping first: without the Pong the subscriber drops the connection as dead.
                // Answered before the lookup below - the subscriber times the next round trip
                // from this very Pong, and anything done ahead of it would read as latency.
                connection.send(&MyNoSqlTcpContract::Pong);

                // Not greeted yet means there is no reader to keep it on - the next ping
                // carries a fresh number anyway.
                if let Some(data_reader) = self.app.data_readers.get_tcp(connection.as_ref()).await
                {
                    data_reader.set_latency(micros);
                }
            }
            MyNoSqlTcpContract::Greeting { name } => {
                my_logger::LOGGER.write_info(
                    "GreetingTcpMessage",
                    "New tcp connection",
                    LogEventCtx::new()
                        .add("Id", connection.id.to_string())
                        .add("Name", name.as_str()),
                );

                self.app
                    .data_readers
                    .add_tcp(connection.clone(), ReaderName::AsReader(name), false)
                    .await;
            }

            MyNoSqlTcpContract::GreetingFromNode {
                node_location,
                node_version,
                compress,
            } => {
                let name = ReaderName::AsNode {
                    location: node_location,
                    version: node_version,
                };
                self.app
                    .data_readers
                    .add_tcp(connection.clone(), name, compress)
                    .await;
            }

            MyNoSqlTcpContract::SetNamespace { namespace } => {
                let data_reader = self.app.data_readers.get_tcp(connection.as_ref()).await;

                let data_reader = match data_reader {
                    Some(data_reader) => data_reader,
                    None => {
                        connection.send(&MyNoSqlTcpContract::Error {
                            message: "SetNamespace is sent before the Greeting".to_string(),
                        });
                        return;
                    }
                };

                if let Err(message) = self.set_namespace(&data_reader, namespace.as_str()).await {
                    my_logger::LOGGER.write_info(
                        "SetNamespaceTcpMessage",
                        message.as_str(),
                        LogEventCtx::new()
                            .add("sessionId", connection.id.to_string())
                            .add("name", data_reader.get_name().to_string())
                            .add("namespace", namespace),
                    );

                    connection.send(&MyNoSqlTcpContract::Error { message });
                }
            }

            MyNoSqlTcpContract::Subscribe { table_name } => {
                if let Some(data_reader) = self.app.data_readers.get_tcp(connection.as_ref()).await
                {
                    let db_namespace = match self.get_namespace_of_subscribe(&data_reader).await {
                        Some(db_namespace) => db_namespace,
                        None => {
                            // No namespace means no table in it — answered with
                            // the empty snapshot a missing table is answered
                            // with, and NOT with an Error contract, which panics
                            // the SDK reader. Subscribing before the first write
                            // is how a reader which starts first behaves.
                            crate::operations::data_readers::send_empty_snapshot(
                                &self.app,
                                data_reader.get_namespace(),
                                data_reader,
                                table_name.as_str(),
                            );

                            return;
                        }
                    };

                    let result = crate::operations::data_readers::subscribe(
                        &self.app,
                        &db_namespace,
                        data_reader,
                        &table_name,
                    )
                    .await;

                    if let Err(err) = result {
                        let session = self.app.data_readers.get_tcp(connection.as_ref()).await;

                        let session_name = if let Some(session) = session {
                            session.get_name().to_string()
                        } else {
                            "".to_string()
                        };

                        let message = format!("Subscribe to table error. Err: {:?}", err);

                        my_logger::LOGGER.write_info(
                            "GreetingTcpMessage",
                            message.as_str(),
                            LogEventCtx::new()
                                .add("sessionId", connection.id.to_string())
                                .add("Name", session_name)
                                .add("TableName", table_name),
                        );

                        connection.send(&MyNoSqlTcpContract::Error { message });
                    }
                }
            }

            MyNoSqlTcpContract::SubscribeAsNode(table_name) => {
                if let Some(data_reader) = self.app.data_readers.get_tcp(connection.as_ref()).await
                {
                    // Resolved without creating even when
                    // `AutoCreateTableOnReaderSubscribe` is on: this path refuses
                    // a missing table below instead of creating it, so it has no
                    // business creating the namespace either. A namespace which
                    // does not exist holds no table, which is the very same
                    // TableNotFound the node already knows how to handle.
                    let db_namespace = match self.get_existing_namespace(&data_reader) {
                        Some(db_namespace) => db_namespace,
                        None => {
                            connection.send(&MyNoSqlTcpContract::TableNotFound(table_name));

                            return;
                        }
                    };

                    let table = db_namespace.db.get_table(table_name.as_str());

                    if table.is_none() {
                        connection.send(&MyNoSqlTcpContract::TableNotFound(table_name));

                        return;
                    }

                    let result = crate::operations::data_readers::subscribe(
                        &self.app,
                        &db_namespace,
                        data_reader.clone(),
                        &table_name,
                    )
                    .await;

                    if let Err(err) = result {
                        let session = self.app.data_readers.get_tcp(connection.as_ref()).await;

                        let session_name = if let Some(session) = session {
                            session.get_name().to_string()
                        } else {
                            "".to_string()
                        };

                        let message =
                            format!("Subscribe to table {} error. Err: {:?}", table_name, err);

                        my_logger::LOGGER.write_info(
                            "SubscribeToTableAsNode",
                            message,
                            LogEventCtx::new()
                                .add("sessionId", connection.id.to_string())
                                .add("name", session_name)
                                .add("tableName", table_name),
                        );
                    }
                }
            }

            MyNoSqlTcpContract::Unsubscribe(table_name) => {
                if let Some(data_reader) = self.app.data_readers.get_tcp(connection.as_ref()).await
                {
                    data_reader.unsubscribe(table_name.as_str()).await;
                }
            }

            MyNoSqlTcpContract::UpdatePartitionsLastReadTime {
                confirmation_id,
                table_name,
                partitions,
            } => {
                let db_table = self
                    .get_table_of_connection(connection, table_name.as_str())
                    .await;

                if let Some((_, db_table)) = db_table {
                    crate::db_operations::update_partitions_last_read_time(
                        &db_table,
                        partitions.iter().map(|x| x.as_str()),
                    )
                    .await;
                }

                connection.send(&MyNoSqlTcpContract::Confirmation { confirmation_id });
            }

            MyNoSqlTcpContract::UpdateRowsLastReadTime {
                confirmation_id,
                table_name,
                partition_key,
                row_keys,
            } => {
                let db_table = self
                    .get_table_of_connection(connection, table_name.as_str())
                    .await;

                if let Some((_, db_table)) = db_table {
                    crate::db_operations::update_row_keys_last_read_access_time(
                        &db_table,
                        &partition_key,
                        row_keys.iter().map(|x| x.as_str()),
                    )
                    .await;
                }

                connection.send(&MyNoSqlTcpContract::Confirmation { confirmation_id });
            }

            MyNoSqlTcpContract::UpdatePartitionsExpirationTime {
                confirmation_id,
                table_name,
                partitions,
            } => {
                let db_table = self
                    .get_table_of_connection(connection, table_name.as_str())
                    .await;

                if let Some((_, db_table)) = &db_table {
                    for (partition_key, set_expiration_time) in partitions {
                        crate::db_operations::update_partition_expiration_time(
                            db_table,
                            partition_key,
                            set_expiration_time,
                        )
                    }
                }

                connection.send(&MyNoSqlTcpContract::Confirmation { confirmation_id });
            }
            MyNoSqlTcpContract::UpdateRowsExpirationTime {
                confirmation_id,
                table_name,
                partition_key,
                row_keys,
                expiration_time,
            } => {
                let db_table = self
                    .get_table_of_connection(connection, table_name.as_str())
                    .await;

                if let Some((db_namespace, db_table)) = &db_table {
                    crate::db_operations::update_rows_expiration_time(
                        db_namespace,
                        db_table,
                        &partition_key,
                        row_keys.iter().map(|x| x.as_str()),
                        expiration_time,
                    )
                }

                connection.send(&MyNoSqlTcpContract::Confirmation { confirmation_id });
            }

            _ => {}
        }
    }
    /*
    async fn handle(
        &self,
        connection_event: ConnectionEvent<MyNoSqlTcpContract, MyNoSqlReaderTcpSerializer, ()>,
    ) {
        match connection_event {
            ConnectionEvent::Connected(_connection) => {

            }
            ConnectionEvent::Disconnected(connection) => {

            }
            ConnectionEvent::Payload {
                connection,
                payload,
            } => self.handle_incoming_packet(payload, connection).await,
        }
    }
     */
}
