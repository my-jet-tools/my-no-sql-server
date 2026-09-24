use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::{
    date_time::DateTimeAsMicroseconds, MyTimerTick, RepeatTimerIteration,
};

use crate::app::AppContext;

pub struct GcHttpSessionsTimer {
    app: Arc<AppContext>,
}

impl GcHttpSessionsTimer {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl MyTimerTick for GcHttpSessionsTimer {
    async fn tick(&self) -> RepeatTimerIteration {
        let now = DateTimeAsMicroseconds::now();

        for data_reader in self.app.data_readers.get_all().await {
            data_reader.ping_http_servers(now).await;
        }
        if let Some(data_readers) = self.app.data_readers.gc_http_sessions(now).await {
            for data_reader in data_readers {
                self.app
                    .metrics
                    .remove_pending_to_sync(&data_reader.connection);
            }
        }

        self.app.http_writers.gc(now).await;

        RepeatTimerIteration::WithInterval
    }
}
