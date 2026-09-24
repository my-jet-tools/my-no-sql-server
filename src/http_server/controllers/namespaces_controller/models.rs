use my_http_server::macros::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct NamespaceContract {
    pub name: String,
    #[serde(rename = "tablesAmount")]
    pub tables_amount: usize,
}

#[derive(MyHttpInput)]
pub struct DeleteNamespaceContract {
    #[http_query(name = "namespace"; description = "Name of the namespace to delete")]
    pub namespace: String,
}
