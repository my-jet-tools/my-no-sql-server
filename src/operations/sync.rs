use std::sync::Arc;

use my_no_sql_sdk::tcp_contracts::MyNoSqlTcpContract;

use crate::{
    app::{AppContext, DbNamespace},
    data_readers::DataReaderConnection,
    db_sync::{NamespaceSyncEvent, SyncEvent},
};

pub fn dispatch(app: &AppContext, db_namespace: &Arc<DbNamespace>, sync_event: SyncEvent) {
    dispatch_by_namespace_name(app, db_namespace.name.clone(), sync_event);
}

/// `dispatch` for an event whose namespace does not exist as an object.
///
/// A `TableFirstInit` is routed by the data reader carried in its payload (see
/// `sync` below), so the namespace of the envelope is a label there and the name
/// is all of it that is needed — which is what makes a reader of a namespace
/// nobody has written to yet answerable at all. Do not reach for this with any
/// other event: those ARE routed by (namespace, table).
pub fn dispatch_by_namespace_name(
    app: &AppContext,
    namespace: my_no_sql_sdk::core::db::DbNamespaceName,
    sync_event: SyncEvent,
) {
    app.sync
        .send(NamespaceSyncEvent::new(namespace, sync_event));
}

pub async fn sync(app: &AppContext, model: &NamespaceSyncEvent) {
    let sync_event = &model.event;

    if let SyncEvent::TableFirstInit(data) = sync_event {
        data.data_reader.set_first_init();

        match &data.data_reader.connection {
            DataReaderConnection::Tcp(tcp_info) => {
                let compressed = tcp_info.is_compressed_data();
                let payloads = crate::data_readers::tcp_connection::tcp_payload_to_send::serialize(
                    sync_event, compressed,
                )
                .await;

                if payloads.len() > 0 {
                    tcp_info.send(payloads.as_slice()).await;
                }
            }
            DataReaderConnection::Http(http_info) => {
                http_info.send(&sync_event).await;
            }
        }

        app.metrics
            .update_pending_to_sync(&data.data_reader.connection);
    } else {
        // Routed by (namespace, table): two namespaces may each hold a table of
        // the same name, and a reader of one of them must never see the other's
        // rows.
        let data_readers = app
            .data_readers
            .get_subscribed_to_table(&model.namespace, sync_event.get_table_name())
            .await;

        if data_readers.is_none() {
            return;
        }
        let data_readers = data_readers.unwrap();

        let mut tcp_contracts_non_compressed: Option<Vec<MyNoSqlTcpContract>> = None;
        let mut tcp_contracts_compressed: Option<Vec<MyNoSqlTcpContract>> = None;

        for data_reader in &data_readers {
            if !data_reader.has_first_init() {
                continue;
            }

            match &data_reader.connection {
                DataReaderConnection::Tcp(connection_info) => {
                    if connection_info.is_compressed_data() {
                        if let Some(payloads) = &tcp_contracts_compressed {
                            connection_info.send(payloads).await;
                        } else {
                            let payloads =
                                crate::data_readers::tcp_connection::tcp_payload_to_send::serialize(
                                    sync_event, true,
                                )
                                .await;

                            if payloads.len() > 0 {
                                connection_info.send(payloads.as_slice()).await;
                                tcp_contracts_compressed = Some(payloads);
                            }
                        }
                    } else {
                        if let Some(to_send) = &tcp_contracts_non_compressed {
                            connection_info.send(to_send).await;
                        } else {
                            let payloads =
                                crate::data_readers::tcp_connection::tcp_payload_to_send::serialize(
                                    sync_event, false,
                                )
                                .await;

                            if payloads.len() > 0 {
                                connection_info.send(&payloads).await;
                                tcp_contracts_non_compressed = Some(payloads);
                            }
                        }
                    }
                }
                DataReaderConnection::Http(http_info) => {
                    http_info.send(&sync_event).await;
                }
            }

            app.metrics.update_pending_to_sync(&data_reader.connection);
        }
    }
}
