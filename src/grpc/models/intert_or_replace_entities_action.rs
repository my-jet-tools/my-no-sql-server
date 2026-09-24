use std::sync::Arc;

use super::{
    table_entity_transport_grpc_contract::TableEntityTransportGrpcContract,
    GrpcContractConvertError,
};
use my_no_sql_sdk::core::{db::DbRow, db_json_entity::JsonTimeStamp};
use prost::DecodeError;

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InsertOrReplaceEntitiesTransactionActionGrpcModel {
    #[prost(string, tag = "1")]
    pub table_name: String,
    #[prost(message, repeated, tag = "2")]
    pub entities: Vec<TableEntityTransportGrpcContract>,
}

impl InsertOrReplaceEntitiesTransactionActionGrpcModel {
    pub fn deserialize(payload: &[u8]) -> Result<Self, DecodeError> {
        prost::Message::decode(payload)
    }

    pub fn to_db_rows(
        &self,
        now: &JsonTimeStamp,
    ) -> Result<Vec<(String, Vec<Arc<DbRow>>)>, GrpcContractConvertError> {
        let mut result = Vec::new();

        for entity in &self.entities {
            let db_rows = entity.to_db_rows(&now)?;

            for db_row in db_rows {
                let partition_key = db_row.get_partition_key();

                match result.binary_search_by(|itm: &(String, Vec<Arc<DbRow>>)| {
                    itm.0.as_str().cmp(partition_key)
                }) {
                    Ok(index) => {
                        result[index].1.push(db_row);
                    }
                    Err(index) => {
                        result.insert(index, (partition_key.to_string(), vec![db_row]));
                    }
                }
            }
        }

        Ok(result)
    }
}
