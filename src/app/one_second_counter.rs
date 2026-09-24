use std::sync::atomic::{AtomicUsize, Ordering};

// Rolling per-second counter: accumulate via `increase`, snapshot once per
// second via `one_second_tick`, read the last snapshot via `get_value`.
pub struct OneSecondCounter {
    intermediary_value: AtomicUsize,
    value: AtomicUsize,
}

impl OneSecondCounter {
    pub fn new() -> Self {
        Self {
            intermediary_value: AtomicUsize::new(0),
            value: AtomicUsize::new(0),
        }
    }

    pub fn increase(&self, delta: usize) {
        self.intermediary_value.fetch_add(delta, Ordering::SeqCst);
    }

    pub fn one_second_tick(&self) {
        let result = self.intermediary_value.swap(0, Ordering::SeqCst);
        self.value.store(result, Ordering::SeqCst);
    }

    pub fn get_value(&self) -> usize {
        self.value.load(Ordering::SeqCst)
    }
}
