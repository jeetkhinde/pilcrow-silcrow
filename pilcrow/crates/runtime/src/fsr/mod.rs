mod baking;
pub mod cache;
mod extractor;
mod handle;
pub mod hub;
mod inspect;
mod live_props;
mod live_trait;
mod macros;
pub mod store;
pub mod watcher;

use std::sync::OnceLock;
static CODEGEN_INVALIDATIONS: OnceLock<Vec<ScheduledInvalidation>> = OnceLock::new();

/// Called by generated `__pilcrow_init()` to register per-field `revalidate`-wired invalidations.
/// Must be called before `start()`. Subsequent calls are silently ignored.
#[doc(hidden)]
pub fn __register_codegen_scheduled_invalidations(items: Vec<ScheduledInvalidation>) {
    let _ = CODEGEN_INVALIDATIONS.set(items);
}

/// Returns the codegen-registered scheduled invalidations for injection into `WatcherConfig`.
pub(crate) fn codegen_scheduled_invalidations() -> Vec<ScheduledInvalidation> {
    CODEGEN_INVALIDATIONS.get().cloned().unwrap_or_default()
}

pub use baking::{find_s_live_slots, inject_fsr_slots};
#[cfg(feature = "live-props-redis")]
pub use cache::RedisCache;
pub use cache::{InvalidatePayload, PatchPayload};
pub use extractor::{extract_live_from_parts, fsr_store_for_handle, register_fsr_store};
pub use handle::FsrHandle;
pub use hub::{
    FsrConnectionCounter, FsrHubConfig, fsr_hub_handler, fsr_hub_handler_or_unavailable,
    fsr_snapshot_handler,
};
pub use inspect::fsr_inspect_handler;
pub use live_props::{DependencyKey, LiveProp};
pub use live_trait::{LiveFieldRegistration, LiveQuery, PilcrowLive};
pub use store::{FsrStore, HitStatus, StaleSlot};
pub use watcher::{
    ScheduledInvalidation, SlotPatch, WatcherConfig, WatcherEventTx, pilcrow_fsr_watcher_tick,
    spawn_embedded_watcher, watcher_tick,
};
