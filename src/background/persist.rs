use std::sync::Arc;

use my_no_sql_sdk::core::rust_extensions::{MyTimerTick, RepeatTimerIteration};

use crate::app::AppContext;

pub struct PersistTimer {
    app: Arc<AppContext>,
}

impl PersistTimer {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl MyTimerTick for PersistTimer {
    async fn tick(&self) -> RepeatTimerIteration {
        // `persist` writes a single queued task and returns whether it did any work.
        // If it did, there may be more in the queue - ask the timer to run us again
        // straight away (with a fresh per-iteration timeout window) instead of trying
        // to drain the whole queue inside one tick under an artificial time budget.
        // When the queue is empty we go back to waiting for the next scheduled tick.
        if crate::operations::persist(&self.app).await {
            RepeatTimerIteration::Immediately
        } else {
            RepeatTimerIteration::WithInterval
        }
    }
}
