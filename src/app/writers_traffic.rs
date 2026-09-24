use std::collections::HashMap;
use std::sync::Mutex;

struct WriterCounter {
    req_intermediary: usize,
    req_value: usize,
    bytes_intermediary: usize,
    bytes_value: usize,
}

// Per-writer-session traffic (requests/sec and bytes/sec). Keyed by the
// `session` id the writer received from the Ping handshake and replays in the
// `session` header. Incremented on every request that carries the header,
// snapshotted once per second. Idle sessions are dropped on tick to keep the
// map bounded to currently-active writers.
pub struct WritersTraffic {
    data: Mutex<HashMap<String, WriterCounter>>,
}

impl WritersTraffic {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }

    pub fn increase(&self, session: &str, body_len: usize) {
        let mut data = self.data.lock().unwrap();
        match data.get_mut(session) {
            Some(counter) => {
                counter.req_intermediary += 1;
                counter.bytes_intermediary += body_len;
            }
            None => {
                data.insert(
                    session.to_string(),
                    WriterCounter {
                        req_intermediary: 1,
                        req_value: 0,
                        bytes_intermediary: body_len,
                        bytes_value: 0,
                    },
                );
            }
        }
    }

    pub fn one_second_tick(&self) {
        let mut data = self.data.lock().unwrap();
        data.retain(|_, counter| {
            counter.req_value = counter.req_intermediary;
            counter.bytes_value = counter.bytes_intermediary;
            counter.req_intermediary = 0;
            counter.bytes_intermediary = 0;
            counter.req_value != 0 || counter.bytes_value != 0
        });
    }

    // Snapshot of currently-active writer sessions: session -> (req/s, bytes/s),
    // only those that had traffic in the last completed second.
    pub fn get_snapshot(&self) -> HashMap<String, (usize, usize)> {
        let data = self.data.lock().unwrap();
        data.iter()
            .filter(|(_, counter)| counter.req_value != 0 || counter.bytes_value != 0)
            .map(|(session, counter)| (session.clone(), (counter.req_value, counter.bytes_value)))
            .collect()
    }
}
