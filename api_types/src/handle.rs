use std::sync::atomic::{AtomicU64, Ordering};

pub struct HandleGenerator {
    next_val: AtomicU64
}

impl HandleGenerator {
    pub fn new() -> Self {
        HandleGenerator {
            next_val: AtomicU64::new(0)
        }
    }

    pub fn generate_handle(
        &mut self
    ) -> u64 {
        self.next_val.fetch_add(1, Ordering::SeqCst)
    }
}