use my_http_server::macros::*;
use std::collections::HashMap;

use my_no_sql_sdk::core::rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::app::AppContext;

#[derive(Debug, Clone, Copy, MyHttpStringEnum)]
pub enum DataSynchronizationPeriod {
    #[http_enum_case(id:0; value:"i"; description="Immediately Persist")]
    Immediately,
    #[http_enum_case(id:1; value:"1"; description="Persist during 1 sec")]
    Sec1,
    #[http_enum_case(id:5; value:"5";  description="Persist during 5 sec"; default)]
    Sec5,
    #[http_enum_case(id:15; value: "15"; description="Persist during 15 sec")]
    Sec15,
    #[http_enum_case(id:30; value: "30"; description="Persist during 30 sec")]
    Sec30,
    #[http_enum_case(id: 60; value: "60"; description="Persist during 1 minute")]
    Min1,
    #[http_enum_case(id:6; value: "a"; description="Sync as soon as CPU schedules task")]
    Asap,
}

impl DataSynchronizationPeriod {
    // `MyHttpInput` expands a bare `default` on an enum field into `Type::create_default()?`, but
    // the `MyHttpStringEnum` derive only emits `Default` (from the case marked `default`). Bridge
    // the two here.
    pub fn create_default() -> Result<Self, my_http_utils::http_input::HttpParseError> {
        Ok(Self::default())
    }

    pub fn get_sync_moment(&self) -> DateTimeAsMicroseconds {
        let mut now = DateTimeAsMicroseconds::now();

        match self {
            DataSynchronizationPeriod::Immediately => {}
            DataSynchronizationPeriod::Sec1 => now.add_seconds(1),
            DataSynchronizationPeriod::Sec5 => now.add_seconds(5),
            DataSynchronizationPeriod::Sec15 => now.add_seconds(15),
            DataSynchronizationPeriod::Sec30 => now.add_seconds(30),
            DataSynchronizationPeriod::Min1 => now.add_minutes(1),
            DataSynchronizationPeriod::Asap => {}
        }

        now
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ClientRequestsSourceData {
    pub locations: Vec<String>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Clone)]
pub enum EventSource {
    #[allow(dead_code)]
    ClientRequest(ClientRequestsSourceData),
    GarbageCollector,
    Subscriber,
    Backup,
}

impl EventSource {
    pub fn as_client_request(app: &AppContext) -> Self {
        let locations = vec![app.settings.location.to_string()];

        let data = ClientRequestsSourceData {
            locations,
            headers: None,
        };

        Self::ClientRequest(data)
    }
}
