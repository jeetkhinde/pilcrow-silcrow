use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use tokio::sync::Semaphore;

use pilcrow_core::ImageConfig;

use super::processor::{self, OutputFormat, TransformParams, cache_path};

#[derive(Clone)]
pub struct ImageState {
    pub config: Arc<ImageConfig>,
    pub semaphore: Arc<Semaphore>,
}

#[derive(Debug, Deserialize)]
pub struct ImageQuery {
    pub src: String,
    #[serde(rename = "w")]
    pub width: Option<u32>,
    #[serde(rename = "h")]
    pub height: Option<u32>,
    #[serde(rename = "q")]
    pub quality: Option<u8>,
    #[serde(rename = "f")]
    pub format: Option<String>,
}

pub async fn image_handler(
    State(state): State<ImageState>,
    Query(q): Query<ImageQuery>,
) -> Response {
    let cfg = &state.config;

    // Validate src — remote URLs are blocked entirely (fetch not yet implemented).
    if q.src.parse::<url::Url>().is_ok() {
        return (StatusCode::NOT_IMPLEMENTED, "Remote image fetch is not supported").into_response();
    }

    let quality = q.quality.unwrap_or(cfg.quality).clamp(1, 100);
    let fmt_str = q.format.as_deref().unwrap_or("auto");
    let format = OutputFormat::from_str(fmt_str).resolve(&cfg.formats);

    let params = TransformParams {
        src: &q.src,
        width: q.width,
        height: q.height,
        quality,
        format,
    };

    let cache_file = cache_path(&cfg.cache_dir, &params);

    // Return cached bytes if present.
    if let Ok(bytes) = tokio::fs::read(&cache_file).await {
        return serve_bytes(bytes, format.mime_type());
    }

    // Acquire concurrency slot.
    let _permit = match state.semaphore.acquire().await {
        Ok(p) => p,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    // Load source image from the configured static root.
    let src_bytes = match load_source(&q.src, &cfg.static_dir).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("image load failed for {}: {e}", q.src);
            return (StatusCode::BAD_REQUEST, "Failed to load source image").into_response();
        }
    };

    // Transform on blocking thread pool.
    let max_w = cfg.max_width;
    let max_h = cfg.max_height;
    let result = tokio::task::spawn_blocking(move || {
        processor::transform(
            &src_bytes,
            params.width,
            params.height,
            quality,
            format,
            max_w,
            max_h,
        )
    })
    .await;

    let (bytes, mime) = match result {
        Ok(Ok(pair)) => pair,
        Ok(Err(e)) => {
            tracing::error!("image transform error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Transform failed").into_response();
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    // Persist to cache (best-effort).
    if let Some(parent) = cache_file.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let _ = tokio::fs::write(&cache_file, &bytes).await;

    serve_bytes(bytes, mime)
}

fn serve_bytes(bytes: Vec<u8>, mime: &'static str) -> Response {
    let mut res = Response::new(Body::from(bytes));
    res.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    res
}

async fn load_source(src: &str, static_dir: &str) -> anyhow::Result<Vec<u8>> {
    let rel = src.trim_start_matches('/');

    // Strip a leading `static_dir/` prefix so callers can pass either
    // "hero.jpg" or "public/hero.jpg" when static_dir is "public".
    let static_prefix = format!("{}/", static_dir.trim_matches('/'));
    let rel = rel.strip_prefix(static_prefix.as_str()).unwrap_or(rel);

    // Reject parent-directory components using platform-agnostic path parsing.
    if std::path::Path::new(rel)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        anyhow::bail!("path traversal not allowed");
    }

    let root = tokio::fs::canonicalize(static_dir)
        .await
        .map_err(|e| anyhow::anyhow!("static_dir unavailable: {e}"))?;
    let candidate = root.join(rel);
    let canonical = tokio::fs::canonicalize(&candidate)
        .await
        .map_err(|e| anyhow::anyhow!("image not found: {e}"))?;

    if !canonical.starts_with(&root) {
        anyhow::bail!("path escapes static root");
    }

    Ok(tokio::fs::read(&canonical).await?)
}
