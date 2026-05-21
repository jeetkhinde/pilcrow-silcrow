/// Background window pre-baking.
///
/// When a paginated route finishes serving cursor window N, call [`trigger`] with the URL
/// for window N+1. A fire-and-forget Tokio task fetches that URL in the background,
/// warming the FSR/HTTP cache before the user scrolls to it.
///
/// ```rust,ignore
/// // In a page handler:
/// pub async fn load(req: Req) -> AppResult<Props> {
///     let tickets = db::tickets_page(&cursor).await?;
///     if let Some(next) = tickets.next_cursor {
///         pilcrow::prebake_next(&format!("/tickets?cursor={next}"));
///     }
///     Ok(Props { tickets })
/// }
/// ```
use std::sync::OnceLock;

static LOCAL_BASE: OnceLock<String> = OnceLock::new();

/// Set the local base URL used by [`trigger`].
///
/// Called once at startup by `start_with_adapter` using the configured host/port.
/// Typically `http://127.0.0.1:{port}`. Has no effect if called more than once.
pub fn set_local_base(base: impl Into<String>) {
    let _ = LOCAL_BASE.set(base.into());
}

/// Spawn a background GET request to `path` on the local server.
///
/// The response is silently discarded — the purpose is to warm the FSR cache so the
/// next real request finds a cache hit. No-op if [`set_local_base`] has not been called.
pub fn trigger(path: impl Into<String>) {
    let Some(base) = LOCAL_BASE.get() else { return };
    let url = format!("{}{}", base, path.into());
    tokio::spawn(async move {
        let _ = reqwest::get(&url).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_is_noop_without_base() {
        // Should not panic; LOCAL_BASE may already be set from another test,
        // so we just verify the call completes without error.
        trigger("/some/path");
    }

    #[tokio::test]
    async fn set_local_base_accepts_valid_url() {
        // set_local_base is idempotent after first call; calling twice is safe.
        set_local_base("http://127.0.0.1:9999");
        // Once set, trigger spawns a task — requires a Tokio runtime.
        trigger("/test");
    }
}
