use super::server::MyNoSqlServerWriterGrpcSerice;
use crate::db_operations::UpdateStatistics;
use crate::db_sync::EventSource;
use crate::http_server::controllers::ToSetExpirationTime;
use crate::mynosqlserver_grpc::writer_server::Writer;
use crate::mynosqlserver_grpc::*;
use futures_core::Stream;
use my_no_sql_sdk::core::db_json_entity::JsonTimeStamp;
use std::pin::Pin;
use tonic::Status;

const OK_GRPC_RESPONSE: i32 = 0;
const TABLE_NOT_FOUND_GRPC_RESPONSE: i32 = 1;
const DB_ROW_NOT_FOUND_GRPC_RESPONSE: i32 = 2;

#[tonic::async_trait]
impl Writer for MyNoSqlServerWriterGrpcSerice {
    type GetRowsStream = Pin<
        Box<
            dyn Stream<Item = Result<TableEntityTransportGrpcContract, Status>>
                + Send
                + Sync
                + 'static,
        >,
    >;

    async fn create_table_if_not_exists(
        &self,
        request: tonic::Request<CreateTableIfNotExistsGrpcRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let request = request.into_inner();

        let db_namespace = self
            .app
            .get_or_create_namespace(request.name_space.as_deref())
            .await
            .map_err(|err| tonic::Status::internal(format!("{:?}", err)))?;

        let event_src = EventSource::as_client_request(self.app.as_ref());

        let max_partitions_amount =
            if let Some(max_partitions_amount) = request.max_partitions_amount {
                Some(max_partitions_amount as usize)
            } else {
                None
            };

        let max_rows_per_partition_amount =
            if let Some(max_rows_per_partition_amount) = request.max_rows_per_partition_amount {
                Some(max_rows_per_partition_amount as usize)
            } else {
                None
            };

        crate::db_operations::write::table::create_if_not_exist(
            &self.app,
            &db_namespace,
            &request.table_name,
            request.persist_table,
            max_partitions_amount,
            max_rows_per_partition_amount,
            event_src,
            crate::app::DEFAULT_PERSIST_PERIOD.get_sync_moment(),
        )
        .await
        .unwrap();

        return Ok(tonic::Response::new(()));
    }

    async fn set_table_attributes(
        &self,
        request: tonic::Request<SetTableAttributesGrpcRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let request = request.into_inner();

        let db_namespace = self
            .app
            .get_or_create_namespace(request.name_space.as_deref())
            .await
            .map_err(|err| tonic::Status::internal(format!("{:?}", err)))?;

        let db_table = crate::db_operations::read::table::get(
            self.app.as_ref(),
            &db_namespace,
            &request.table_name,
        )
        .await
        .unwrap();

        let event_src = EventSource::as_client_request(self.app.as_ref());

        let max_partitions_amount =
            if let Some(max_partitions_amount) = request.max_partitions_amount {
                Some(max_partitions_amount as usize)
            } else {
                None
            };

        let max_rows_per_partition_amount =
            if let Some(max_rows_per_partition_amount) = request.max_rows_per_partition_amount {
                Some(max_rows_per_partition_amount as usize)
            } else {
                None
            };

        let persist = db_table.get_persist_table();
        crate::db_operations::write::table::set_table_attributes(
            &self.app,
            &db_namespace,
            db_table,
            persist,
            max_partitions_amount,
            max_rows_per_partition_amount,
            event_src,
        )
        .await
        .unwrap();

        return Ok(tonic::Response::new(()));
    }

    async fn get_rows(
        &self,
        request: tonic::Request<GetEntitiesGrpcRequest>,
    ) -> Result<tonic::Response<Self::GetRowsStream>, tonic::Status> {
        let request = request.into_inner();

        let db_namespace = self
            .app
            .get_or_create_namespace(request.name_space.as_deref())
            .await
            .map_err(|err| tonic::Status::internal(format!("{:?}", err)))?;

        let db_table = crate::db_operations::read::table::get(
            self.app.as_ref(),
            &db_namespace,
            &request.table_name,
        )
        .await
        .unwrap();

        let partition_key = request.partition_key.as_ref();
        let row_key = request.row_key.as_ref();

        let limit = if let Some(limit) = request.limit {
            Some(limit as usize)
        } else {
            None
        };

        let skip = if let Some(skip) = request.skip {
            Some(skip as usize)
        } else {
            None
        };

        let (tx, rx) = tokio::sync::mpsc::channel(4);

        let date_time = JsonTimeStamp::now();

        let update_statistics = UpdateStatistics {
            update_partition_last_read_access_time: if let Some(value) =
                request.update_partition_last_read_time
            {
                value
            } else {
                false
            },
            update_rows_last_read_access_time: if let Some(value) =
                request.update_rows_last_read_time
            {
                value
            } else {
                false
            },
            update_partition_expiration_time: request
                .set_partition_expiration_time
                .to_set_expiration_time(),
            update_rows_expiration_time: request.set_rows_expiration_time.to_set_expiration_time(),
        };

        let db_rows = crate::db_operations::read::get_rows_as_vec::execute(
            &self.app,
            &db_table,
            partition_key,
            row_key,
            limit,
            skip,
            &date_time,
            update_statistics,
        )
        .await
        .unwrap();

        tokio::spawn(async move {
            for db_row in db_rows {
                let grpc = TableEntityTransportGrpcContract {
                    content_type: 0,
                    content: db_row.to_vec(),
                };

                tx.send(Ok(grpc)).await.unwrap();
            }
        });

        Ok(tonic::Response::new(Box::pin(
            tokio_stream::wrappers::ReceiverStream::new(rx),
        )))
    }

    async fn get_row(
        &self,
        request: tonic::Request<GetEntityGrpcRequest>,
    ) -> Result<tonic::Response<GetDbRowGrpcResponse>, tonic::Status> {
        let request = request.into_inner();

        let db_namespace = self
            .app
            .get_or_create_namespace(request.name_space.as_deref())
            .await
            .map_err(|err| tonic::Status::internal(format!("{:?}", err)))?;

        let db_table = crate::db_operations::read::table::get(
            self.app.as_ref(),
            &db_namespace,
            &request.table_name,
        )
        .await;

        if let Err(_) = db_table {
            let result = GetDbRowGrpcResponse {
                response_code: TABLE_NOT_FOUND_GRPC_RESPONSE,
                entity: None,
            };

            return Ok(tonic::Response::new(result));
        }

        let db_table = db_table.unwrap();

        let date_time = JsonTimeStamp::now();

        let update_statistics = UpdateStatistics {
            update_partition_last_read_access_time: if let Some(value) =
                request.update_partition_last_read_time
            {
                value
            } else {
                false
            },
            update_rows_last_read_access_time: if let Some(value) =
                request.update_rows_last_read_time
            {
                value
            } else {
                false
            },
            update_partition_expiration_time: request
                .set_partition_expiration_time
                .to_set_expiration_time(),
            update_rows_expiration_time: request.set_rows_expiration_time.to_set_expiration_time(),
        };

        let db_row = crate::db_operations::read::get_rows_as_vec::get_as_partition_key_and_row_key(
            &db_table,
            &request.partition_key,
            &request.row_key,
            &date_time,
            update_statistics,
        )
        .await;

        if db_row.is_none() {
            let result = GetDbRowGrpcResponse {
                response_code: DB_ROW_NOT_FOUND_GRPC_RESPONSE,
                entity: None,
            };
            return Ok(tonic::Response::new(result));
        }

        let entity = TableEntityTransportGrpcContract {
            content_type: 0,
            content: db_row.unwrap().to_vec(),
        };

        let result = GetDbRowGrpcResponse {
            response_code: OK_GRPC_RESPONSE,
            entity: Some(entity),
        };

        Ok(tonic::Response::new(result))
    }

    async fn post_transaction_actions(
        &self,
        request: tonic::Request<TransactionPayloadGrpcRequest>,
    ) -> Result<tonic::Response<TransactionGrpcResponse>, tonic::Status> {
        println!("PostTransaction");
        let request = request.into_inner();

        let db_namespace = self
            .app
            .get_or_create_namespace(request.name_space.as_deref())
            .await
            .map_err(|err| tonic::Status::internal(format!("{:?}", err)))?;

        let transaction_id = if let Some(transaction_id) = &request.transaction_id {
            transaction_id.to_string()
        } else {
            self.app
                .active_transactions
                .issue_new(db_namespace.name.clone())
                .await
        };

        let mut events = Vec::new();
        let now = JsonTimeStamp::now();
        for action in &request.actions {
            let action = super::models::deserializer::deserialize(
                action.transaction_type(),
                &action.payload,
                &now,
            )
            .unwrap();

            events.push(action);
        }

        self.app
            .active_transactions
            .add_events(&transaction_id, events)
            .await;

        if request.commit {
            let event_src = EventSource::as_client_request(self.app.as_ref());
            crate::db_operations::transactions::commit(
                self.app.as_ref(),
                &transaction_id,
                event_src,
                crate::app::DEFAULT_PERSIST_PERIOD.get_sync_moment(),
                now.date_time,
            )
            .await
            .unwrap();
        }

        let result = TransactionGrpcResponse {
            result: 0,
            id: transaction_id,
        };

        println!("PostTransaction Done");
        return Ok(tonic::Response::new(result));
    }

    async fn cancel_transaction(
        &self,
        request: tonic::Request<CancelTransactionGrpcRequest>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        println!("CancelTransaction");
        let request = request.into_inner();

        // No namespace here on purpose: the transaction id resolves to the
        // transaction, which already knows the namespace it was started in.
        self.app.active_transactions.remove(&request.id).await;

        return Ok(tonic::Response::new(()));
    }
}
