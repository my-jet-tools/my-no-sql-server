use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::{MyTimerTick, RepeatTimerIteration};

use crate::app::AppContext;

/// Enforces `MaxBackupsToKeep`.
///
/// A timer of its own, not a second step of `BackupDb`: registered timers run as
/// separate tasks, so whatever happens while snapshots are being written can not
/// stop old ones from being collected. As one tick it silently did not run for
/// as long as writing a snapshot kept failing, which reads exactly like
/// "MaxBackupsToKeep does not work".
pub struct GcBackupsTimer {
    app: Arc<AppContext>,
}

impl GcBackupsTimer {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl MyTimerTick for GcBackupsTimer {
    async fn tick(&self) -> RepeatTimerIteration {
        crate::operations::backup::gc_backups(&self.app).await;

        RepeatTimerIteration::WithInterval
    }
}
