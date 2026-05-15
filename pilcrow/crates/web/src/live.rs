//! Developer-facing FSR (Field-Selective Rendering) surface.
//!
//! Import everything with: `use pilcrow::live::*;`

#[cfg(feature = "live-props")]
pub use runtime::fsr::{DependencyKey, LiveProp, LiveQuery, PilcrowLive};

// Re-export macros from their defining crates.
// `fsr_dep!` and `live_query!` are `#[macro_export]` macros in pilcrow-runtime.
#[cfg(feature = "live-props")]
pub use runtime::fsr_dep as dep;
#[cfg(feature = "live-props")]
pub use runtime::live_query;

// `fsr_invalidate!` is a proc-macro from pilcrow-macros.
pub use pilcrow_macros::fsr_invalidate as invalidate;
