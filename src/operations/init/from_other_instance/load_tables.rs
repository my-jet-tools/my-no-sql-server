use flurl::FlUrl;
use my_no_sql_sdk::core::db::DbTableAttributes;
use my_no_sql_sdk::server::rust_extensions::date_time::DateTimeAsMicroseconds;
use serde::*;

use crate::operations::init::TableAttributeInitContract;

#[derive(Debug, Serialize, Deserialize)]
pub struct TableMyNoSqlServerContract {
    pub name: String,
    pub persist: Option<bool>,
    #[serde(rename = "maxPartitionsAmount")]
    pub max_partitions_amount: Option<i64>,
    pub max_rows_per_partition_amount: Option<i64>,
    #[serde(default)]
    pub compressed: Option<bool>,
    pub created: Option<String>,
}

impl TableAttributeInitContract for TableMyNoSqlServerContract {
    fn into(
        self,
    ) -> (
        my_no_sql_sdk::core::db::DbTableName,
        my_no_sql_sdk::core::db::DbTableAttributes,
    ) {
        let created = if let Some(created) = self.created.as_ref() {
            match DateTimeAsMicroseconds::from_str(created) {
                Some(x) => x,
                None => DateTimeAsMicroseconds::now(),
            }
        } else {
            DateTimeAsMicroseconds::now()
        };

        let attributes = DbTableAttributes {
            persist: self.persist.unwrap_or(false),
            max_partitions_amount: self.max_partitions_amount.map(|x| x as usize),
            max_rows_per_partition_amount: self.max_rows_per_partition_amount.map(|x| x as usize),
            compressed: self.compressed.unwrap_or(false),
            created,
        };

        (self.name.into(), attributes)
    }
}

pub async fn load_tables(url: &str) -> Vec<TableMyNoSqlServerContract> {
    let mut response = FlUrl::new(url)
        .append_path_segment("api")
        .append_path_segment("Tables")
        .append_path_segment("List")
        .get()
        .await
        .unwrap();

    let body = response.get_body_as_slice().await.unwrap();

    serde_json::from_slice(body).unwrap()
}
