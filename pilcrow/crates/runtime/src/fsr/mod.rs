mod baking;
pub mod cache;
mod extractor;
pub mod hub;
mod inspect;
mod live_props;
mod live_trait;
mod macros;
pub mod store;
pub mod watcher;

pub use baking::{find_s_live_slots, inject_fsr_slots};
pub use extractor::{extract_live_from_parts, register_fsr_store};
pub use hub::{
    fsr_hub_handler,
    fsr_hub_handler_or_unavailable,
    fsr_snapshot_handler,
    FsrConnectionCounter,
    FsrHubConfig,
};
pub use inspect::fsr_inspect_handler;
pub use live_props::{DependencyKey, LiveProp};
pub use live_trait::{LiveFieldRegistration, LiveQuery, PilcrowLive};
pub use cache::{InvalidatePayload, PatchPayload};
#[cfg(feature = "live-props-redis")]
pub use cache::RedisCache;
pub use store::{FsrStore, StaleSlot};
pub use watcher::{
    SlotPatch, WatcherConfig, WatcherEventTx, pilcrow_fsr_watcher_tick, spawn_embedded_watcher,
    watcher_tick,
};
