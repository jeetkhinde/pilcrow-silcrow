// ./crates/pilcrow/src/assets.rs

use std::sync::atomic::{AtomicBool, Ordering};

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

pub const SILCROW_JS: &str = include_str!("../../assets/silcrow.js");
pub const REACT_ISLANDS_JS: &str = include_str!("../../assets/react-islands.js");
pub const SOLID_ISLANDS_JS: &str = include_str!("../../assets/solid-islands.js");

/// When `true`, `script_tag()` returns an inline `<script>` block instead of a `<script src>` tag.
/// Set at startup from `Pilcrow.toml` `[client] inline_runtime = true`.
static INLINE_RUNTIME: AtomicBool = AtomicBool::new(false);

/// Called once at startup by `start_with_adapter`. Sets the process-wide inline-runtime flag
/// read by `script_tag()` on every template render.
pub fn set_inline_runtime(inline: bool) {
    INLINE_RUNTIME.store(inline, Ordering::Relaxed);
}

pub async fn serve_silcrow_js() -> Response {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        SILCROW_JS,
    )
        .into_response()
}

pub fn silcrow_js_path() -> String {
    let hash = crc32fast::hash(SILCROW_JS.as_bytes());
    format!("/__pilcrow/runtime/silcrow.{hash:08x}.js")
}

pub fn react_islands_js_path() -> String {
    let hash = crc32fast::hash(REACT_ISLANDS_JS.as_bytes());
    format!("/_pilcrow/react-islands.{hash:08x}.js")
}

/// Returns the appropriate `<script>` element for the Silcrow runtime.
///
/// When `inline_runtime = true` is set in `Pilcrow.toml`, returns an inline
/// `<script>` block with the full Silcrow JS content embedded.
/// Otherwise returns `<script src="/__pilcrow/runtime/silcrow.{hash}.js" defer></script>`.
///
/// Use in Askama templates: `{{ pilcrow_web::assets::assets::script_tag()|safe }}`
pub fn script_tag() -> String {
    if INLINE_RUNTIME.load(Ordering::Relaxed) {
        format!("<script>{SILCROW_JS}</script>")
    } else {
        format!(r#"<script src="{}" defer></script>"#, silcrow_js_path())
    }
}

pub async fn serve_react_islands_js() -> Response {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        REACT_ISLANDS_JS,
    )
        .into_response()
}

pub fn solid_islands_js_path() -> String {
    let hash = crc32fast::hash(SOLID_ISLANDS_JS.as_bytes());
    format!("/_pilcrow/solid-islands.{hash:08x}.js")
}

pub async fn serve_solid_islands_js() -> Response {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        SOLID_ISLANDS_JS,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silcrow_js_path_under_pilcrow_runtime() {
        let path = silcrow_js_path();
        assert!(
            path.starts_with("/__pilcrow/runtime/"),
            "silcrow path should start with /__pilcrow/runtime/: {path}"
        );
        assert!(
            path.ends_with(".js"),
            "silcrow path should end with .js: {path}"
        );
        assert!(
            !path.contains("/_silcrow/"),
            "/_silcrow/ prefix should be gone: {path}"
        );
    }

    #[test]
    fn script_tag_external_by_default() {
        set_inline_runtime(false);
        let tag = script_tag();
        assert!(tag.contains("src="), "external: tag should have src attribute: {tag}");
        assert!(tag.contains("/__pilcrow/runtime/"), "external: should use new path: {tag}");
        assert!(!tag.contains(SILCROW_JS), "external: should not embed JS: {tag}");
    }

    #[test]
    fn script_tag_inline_when_flag_set() {
        set_inline_runtime(true);
        let tag = script_tag();
        assert!(!tag.contains("src="), "inline: tag should not have src attribute: {tag}");
        assert!(tag.starts_with("<script>"), "inline: should be a plain <script> tag: {tag}");
        set_inline_runtime(false); // reset for other tests
    }
}
