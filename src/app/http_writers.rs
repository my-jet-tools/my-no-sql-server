use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use my_no_sql_sdk::server::rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct WriterInfo {
    pub name: String,
    /// Namespace the writer works in, taken from the `ns` header of its ping.
    /// A writer that names none works in the default namespace.
    pub namespace: String,
    pub version: String,
    pub last_ping: DateTimeAsMicroseconds,
    pub tables: Vec<String>,
    pub addr: String,
}

// Writers are keyed by their `session` id. The server issues the id on the
// first (header-less) ping and returns it; the client then replays it in the
// `session` header on every subsequent request, so the same client always
// maps back to the same writer row.
pub struct HttpWriters {
    data: Mutex<BTreeMap<String, WriterInfo>>,
    next_session_id: AtomicU64,
}

impl HttpWriters {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(BTreeMap::new()),
            next_session_id: AtomicU64::new(0),
        }
    }

    fn generate_session_id(&self) -> String {
        let id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        format!("Writer-{}", id)
    }

    // Refreshes the writer identified by `session`, or creates a new entry,
    // returning the resulting session id:
    //   * `Some(s)` -> use it as-is (known -> refresh; unknown -> re-adopt the
    //                  id the client still holds after a server restart/GC).
    //   * `None`    -> first ping of a fresh writer: issue a new `Writer-{n}`
    //                  and return it; the client adopts it and replays it.
    pub async fn get_or_create(
        &self,
        session: Option<&str>,
        namespace: &str,
        name: &str,
        version: &str,
        tables: impl Iterator<Item = &str>,
        addr: String,
        now: DateTimeAsMicroseconds,
    ) -> String {
        let mut data = self.data.lock().await;

        let session_id = match session {
            Some(session) => session.to_string(),
            None => self.generate_session_id(),
        };

        let writer_info = data
            .entry(session_id.clone())
            .or_insert_with(|| WriterInfo {
                name: name.to_string(),
                namespace: namespace.to_string(),
                version: version.to_string(),
                last_ping: now,
                tables: Vec::new(),
                addr: addr.clone(),
            });

        writer_info.last_ping = now;
        writer_info.name = name.to_string();
        writer_info.namespace = namespace.to_string();
        writer_info.version = version.to_string();
        writer_info.tables = tables.map(|x| x.to_string()).collect();
        writer_info.addr = addr;

        session_id
    }

    pub async fn get<TResult>(
        &self,
        convert: impl Fn(&str, &WriterInfo) -> TResult,
    ) -> Vec<TResult> {
        let data = self.data.lock().await;

        let mut result = Vec::with_capacity(data.len());

        for (session_id, itm) in data.iter() {
            let itm = convert(session_id, itm);
            result.push(itm);
        }

        result
    }

    pub async fn gc(&self, now: DateTimeAsMicroseconds) {
        let mut data = self.data.lock().await;
        let mut to_remove = Vec::new();
        for (session_id, writer_info) in data.iter() {
            if now.duration_since(writer_info.last_ping).get_full_minutes() > 1 {
                to_remove.push(session_id.clone());
            }
        }

        for session_id in to_remove {
            data.remove(&session_id);
        }
    }
}
