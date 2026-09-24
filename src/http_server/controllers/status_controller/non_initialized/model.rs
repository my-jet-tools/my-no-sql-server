use crate::app::AppContext;
use my_http_server::macros::*;
use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, MyHttpObjectStructure)]
pub struct NonInitializedModel {
    #[serde(rename = "tablesTotal")]
    tables_total: usize,
    #[serde(rename = "tablesLoaded")]
    tables_loaded: usize,
    #[serde(rename = "currentTable")]
    current_table: Option<String>,
    error: Option<String>,
    #[serde(rename = "initializingSeconds")]
    loading_time: i64,
}

impl NonInitializedModel {
    pub async fn new(app: &AppContext) -> Self {
        let now = DateTimeAsMicroseconds::now();

        let state = app.init_state.clone().await;

        Self {
            tables_total: state.total_tables,
            tables_loaded: state.loaded,
            current_table: state.current_table,
            error: state.error,

            loading_time: now.seconds_before(app.created),
        }
    }
}
