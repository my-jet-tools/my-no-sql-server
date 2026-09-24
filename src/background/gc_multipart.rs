use std::{sync::Arc, time::Duration};

use my_no_sql_sdk::core::rust_extensions::{
    date_time::DateTimeAsMicroseconds, MyTimerTick, RepeatTimerIteration,
};

use crate::app::AppContext;

pub struct GcMultipart {
    app: Arc<AppContext>,
}

impl GcMultipart {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl MyTimerTick for GcMultipart {
    async fn tick(&self) -> RepeatTimerIteration {
        let multipart_timeout = Duration::from_secs(60);

        let now = DateTimeAsMicroseconds::now();

        self.app.multipart_list.gc(now, multipart_timeout).await;

        RepeatTimerIteration::WithInterval
    }
}
