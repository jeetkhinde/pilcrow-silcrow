use std::sync::atomic::{AtomicU64, Ordering};

static RENDER_COUNT: AtomicU64 = AtomicU64::new(0);

/// Increment and return the new value. Call once per load() invocation.
pub fn increment() -> u64 {
    RENDER_COUNT.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn current() -> u64 {
    RENDER_COUNT.load(Ordering::Relaxed)
}
