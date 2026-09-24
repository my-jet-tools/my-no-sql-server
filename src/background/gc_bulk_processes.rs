use std::{sync::Arc, time::Duration};

use my_no_sql_sdk::core::rust_extensions::{
    date_time::DateTimeAsMicroseconds, MyTimerTick, RepeatTimerIteration,
};

use crate::app::AppContext;

/// A chunked upload holds a full copy of the table's new content in memory, so
/// an abandoned process is an expensive leak. Anything untouched for longer
/// than this is dropped.
const BULK_PROCESS_TIMEOUT: Duration = Duration::from_secs(120);

pub struct GcBulkProcesses {
    app: Arc<AppContext>,
}

impl GcBulkProcesses {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl MyTimerTick for GcBulkProcesses {
    async fn tick(&self) -> RepeatTimerIteration {
        let now = DateTimeAsMicroseconds::now();

        self.app
            .active_bulk_processes
            .gc(now, BULK_PROCESS_TIMEOUT)
            .await;

        RepeatTimerIteration::WithInterval
    }
}
