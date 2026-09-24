use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};

use my_no_sql_sdk::core::rust_extensions::{
    date_time::DateTimeAsMicroseconds, MyTimerTick, RepeatTimerIteration,
};

use crate::app::AppContext;

/// How often the persistence backend is compacted.
const VACUUM_INTERVAL_SECS: u64 = 60 * 60;

/// Wakes up every minute and compacts the persistence of every namespace once
/// an hour has passed since the previous run, dropping fully-freed page-files.
/// The last-run timestamp is kept in memory, so after a restart the first vacuum
/// happens an hour later.
pub struct VacuumTimer {
    app: Arc<AppContext>,
    last_vacuum_unix_micros: AtomicI64,
}

impl VacuumTimer {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self {
            app,
            last_vacuum_unix_micros: AtomicI64::new(
                DateTimeAsMicroseconds::now().unix_microseconds,
            ),
        }
    }
}

#[async_trait::async_trait]
impl MyTimerTick for VacuumTimer {
    async fn tick(&self) -> RepeatTimerIteration {
        let now = DateTimeAsMicroseconds::now();
        let last_vacuum =
            DateTimeAsMicroseconds::new(self.last_vacuum_unix_micros.load(Ordering::Relaxed));

        if now
            .duration_since(last_vacuum)
            .as_positive_or_zero()
            .as_secs()
            < VACUUM_INTERVAL_SECS
        {
            return RepeatTimerIteration::WithInterval;
        }

        println!("Running persistence vacuum...");
        for db_namespace in self.app.namespaces.get_all() {
            db_namespace.repo.vacuum().await;
        }
        self.last_vacuum_unix_micros
            .store(now.unix_microseconds, Ordering::Relaxed);
        println!("Persistence vacuum completed");

        RepeatTimerIteration::WithInterval
    }
}
