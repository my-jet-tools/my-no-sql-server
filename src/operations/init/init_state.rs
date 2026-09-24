use tokio::sync::Mutex;

#[derive(Clone)]
pub struct InitStateInner {
    pub total_tables: usize,
    pub loaded: usize,
    pub current_table: Option<String>,
    pub error: Option<String>,
}

pub struct InitState {
    inner: Mutex<InitStateInner>,
}

impl InitState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(InitStateInner {
                total_tables: 0,
                loaded: 0,
                current_table: None,
                error: None,
            }),
        }
    }

    pub async fn clone(&self) -> InitStateInner {
        let inner = self.inner.lock().await;
        inner.clone()
    }
}
