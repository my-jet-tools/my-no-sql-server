use prost::DecodeError;

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CleanTableTransactionActionGrpcModel {
    #[prost(string, tag = "1")]
    pub table_name: ::prost::alloc::string::String,
}

impl CleanTableTransactionActionGrpcModel {
    pub fn deserialize(payload: &[u8]) -> Result<Self, DecodeError> {
        prost::Message::decode(payload)
    }
}
