//! Experimental baked-page model and storage types.
//!
//! This module intentionally stays library-only for now. It lets route/page code
//! declare bake timing, store durable artifacts, and read fresh artifacts without
//! changing Pilcrow's current SSR/load request path.

mod inject;
mod model;
mod patch;
mod serving;
mod store;

pub use crate::deferred::{BakedField, BakedProp, PatchDelay};
pub use inject::inject_slots;
pub use model::{
    BakeEligibility, BakedPage, BakedPagePaths, BakedSlot, BakedSlotKind, DependencyConfig,
    DependencyKey, StaleState,
};
pub use patch::{BakedPatchOutcome, BakedPatchRegistry};
pub use serving::{BakedRenderedOutput, BakedServeOutcome, BakedServeState};
pub use store::{BakedPageStore, ReverseIndex};
