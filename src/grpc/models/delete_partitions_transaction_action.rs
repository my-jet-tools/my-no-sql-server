use prost::DecodeError;
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeletePartitionsTransactionActionGrpcModel {
    #[prost(string, tag = "1")]
    pub table_name: ::prost::alloc::string::String,
    #[prost(string, repeated, tag = "2")]
    pub partition_keys: Vec<::prost::alloc::string::String>,
}

impl DeletePartitionsTransactionActionGrpcModel {
    pub fn deserialize(payload: &[u8]) -> Result<Self, DecodeError> {
        prost::Message::decode(payload)
    }
}
