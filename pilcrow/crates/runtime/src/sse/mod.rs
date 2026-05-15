// src/sse/mod.rs
mod ext;
mod macros;
mod server_sent_events;
mod watch;

mod interval;
pub use ext::PilcrowStreamExt;
pub use interval::interval;
pub(crate) use macros::serialize_or_null;
pub use server_sent_events::{EmitError, SilcrowEvent, SseEmitter, SseRoute, sse_raw, sse_stream};
pub use watch::watch;
