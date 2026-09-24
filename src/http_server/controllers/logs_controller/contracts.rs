use my_http_server::macros::*;

#[derive(MyHttpInput)]
pub struct GetLogsByProcess {
    #[http_path(description:"Id of process")]
    pub process_name: String,
}

#[derive(MyHttpInput)]
pub struct GetLogsByTableName {
    #[http_path(description:"Table name")]
    pub table_name: String,
}
