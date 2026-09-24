use std::time::Duration;

use my_no_sql_sdk::server::rust_extensions::date_time::DateTimeAsMicroseconds;

#[derive(Debug, Default, Clone)]
pub struct PersistMetrics {
    pub last_persist_time: Option<DateTimeAsMicroseconds>,
    pub next_persist_time: Option<DateTimeAsMicroseconds>,
    pub persist_amount: usize,
    pub last_persist_duration: Vec<usize>,
}

impl PersistMetrics {
    pub fn update(&mut self, last_persist_time: DateTimeAsMicroseconds, duration: Duration) {
        self.last_persist_time = Some(last_persist_time);
        self.last_persist_duration
            .push(duration.as_micros() as usize);

        if self.last_persist_duration.len() > 100 {
            self.last_persist_duration.remove(0);
        }
    }
}
