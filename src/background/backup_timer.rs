use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::{MyTimerTick, RepeatTimerIteration};

use crate::app::AppContext;

pub struct BackupTimer {
    app: Arc<AppContext>,
}

impl BackupTimer {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl MyTimerTick for BackupTimer {
    async fn tick(&self) -> RepeatTimerIteration {
        // Failures are already reported by save_backup itself, per namespace.
        // Collecting old snapshots is the GcBackups timer's job — deliberately
        // not this tick's second half.
        let _ = crate::operations::backup::save_backup(&self.app, false).await;

        RepeatTimerIteration::WithInterval
    }
}
