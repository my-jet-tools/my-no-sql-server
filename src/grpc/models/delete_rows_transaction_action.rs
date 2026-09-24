use prost::DecodeError;

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteRowsTransactionActionGrpcModel {
    #[prost(string, tag = "1")]
    pub table_name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub partition_key: ::prost::alloc::string::String,
    #[prost(string, repeated, tag = "3")]
    pub row_keys: Vec<::prost::alloc::string::String>,
}

impl DeleteRowsTransactionActionGrpcModel {
    pub fn deserialize(payload: &[u8]) -> Result<Self, DecodeError> {
        prost::Message::decode(payload)
    }
}
