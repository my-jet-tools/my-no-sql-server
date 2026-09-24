use std::collections::VecDeque;

use tokio::sync::Mutex;

const MAX_DATA_LEN: usize = 120;

pub struct SendPerSecond {
    data: Mutex<VecDeque<usize>>,
}

impl SendPerSecond {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(VecDeque::new()),
        }
    }

    pub async fn add(&self, value: usize) {
        let mut write_access = self.data.lock().await;
        write_access.push_back(value);

        while write_access.len() > MAX_DATA_LEN {
            write_access.pop_front();
        }
    }

    pub async fn get_snapshot(&self) -> Vec<usize> {
        let read_access = self.data.lock().await;

        let mut result = Vec::with_capacity(read_access.len());

        for value in read_access.iter() {
            result.push(*value);
        }

        result
    }
}
