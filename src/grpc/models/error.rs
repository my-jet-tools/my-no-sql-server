use my_no_sql_sdk::core::db_json_entity::DbEntityParseFail;

#[derive(Debug)]
pub enum GrpcContractConvertError {
    #[allow(dead_code)]
    DbEntityParseFail(DbEntityParseFail),
    #[allow(dead_code)]
    ProstDecodeError(prost::DecodeError),
}

impl From<DbEntityParseFail> for GrpcContractConvertError {
    fn from(src: DbEntityParseFail) -> Self {
        Self::DbEntityParseFail(src)
    }
}

impl From<prost::DecodeError> for GrpcContractConvertError {
    fn from(src: prost::DecodeError) -> Self {
        Self::ProstDecodeError(src)
    }
}
