/// Tests for `SilcrowEvent`, `SseEmitter`, and `sse_stream`.
use axum::response::IntoResponse;
use http_body_util::BodyExt;
use runtime::{EmitError, SilcrowEvent, sse_stream};

async fn collect_sse_body(sse: impl IntoResponse) -> String {
    let bytes = sse
        .into_response()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

// ── content-type ──────────────────────────────────────────────

#[tokio::test]
async fn sse_stream_has_event_stream_content_type() {
    let resp = sse_stream(|_| async move { Ok(()) }).into_response();
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.contains("text/event-stream"), "got: {ct}");
}

// ── patch ──────────────────────────────────────────────────────

#[tokio::test]
async fn sse_patch_event_body_contains_target_and_data() {
    let body = collect_sse_body(sse_stream(|emitter| async move {
        emitter
            .send(SilcrowEvent::patch(serde_json::json!({"count": 1}), "#app"))
            .await
    }))
    .await;
    assert!(body.contains("patch"), "event name missing:\n{body}");
    assert!(body.contains("#app"), "target missing:\n{body}");
    assert!(body.contains("\"count\""), "data key missing:\n{body}");
}

// ── html ───────────────────────────────────────────────────────

#[tokio::test]
async fn sse_html_event_body_contains_target_and_markup() {
    let body = collect_sse_body(sse_stream(|emitter| async move {
        emitter
            .send(SilcrowEvent::html("<p>hello</p>", "#content"))
            .await
    }))
    .await;
    assert!(body.contains("html"), "event name missing:\n{body}");
    assert!(body.contains("#content"), "target missing:\n{body}");
    assert!(body.contains("<p>hello</p>"), "markup missing:\n{body}");
}

// ── invalidate ────────────────────────────────────────────────

#[tokio::test]
async fn sse_invalidate_event_body_contains_target() {
    let body = collect_sse_body(sse_stream(|emitter| async move {
        emitter.send(SilcrowEvent::invalidate("#list")).await
    }))
    .await;
    assert!(body.contains("invalidate"), "event name missing:\n{body}");
    assert!(body.contains("#list"), "target missing:\n{body}");
}

// ── navigate ──────────────────────────────────────────────────

#[tokio::test]
async fn sse_navigate_event_body_contains_path() {
    let body = collect_sse_body(sse_stream(|emitter| async move {
        emitter.send(SilcrowEvent::navigate("/dashboard")).await
    }))
    .await;
    assert!(body.contains("navigate"), "event name missing:\n{body}");
    assert!(body.contains("/dashboard"), "path missing:\n{body}");
}

// ── custom ────────────────────────────────────────────────────

#[tokio::test]
async fn sse_custom_event_body_contains_event_name_and_data() {
    let body = collect_sse_body(sse_stream(|emitter| async move {
        emitter
            .send(SilcrowEvent::custom(
                "user-joined",
                serde_json::json!({"name": "alice"}),
            ))
            .await
    }))
    .await;
    assert!(body.contains("custom"), "event type missing:\n{body}");
    assert!(body.contains("user-joined"), "event name missing:\n{body}");
    assert!(body.contains("alice"), "data missing:\n{body}");
}

// ── with_id ───────────────────────────────────────────────────

#[tokio::test]
async fn sse_event_with_id_appears_in_body() {
    let body = collect_sse_body(sse_stream(|emitter| async move {
        emitter
            .send(SilcrowEvent::navigate("/next").with_id("42"))
            .await
    }))
    .await;
    assert!(body.contains("42"), "id missing:\n{body}");
}

// ── json alias ────────────────────────────────────────────────

#[tokio::test]
async fn sse_json_is_alias_for_patch() {
    let patch_body = collect_sse_body(sse_stream(|emitter| async move {
        emitter
            .send(SilcrowEvent::patch(serde_json::json!({"v": 1}), "#w"))
            .await
    }))
    .await;
    let json_body = collect_sse_body(sse_stream(|emitter| async move {
        emitter
            .send(SilcrowEvent::json(serde_json::json!({"v": 1}), "#w"))
            .await
    }))
    .await;
    assert_eq!(patch_body, json_body);
}

// ── emitter.json() ────────────────────────────────────────────

#[tokio::test]
async fn sse_emitter_json_sends_patch_event() {
    let body = collect_sse_body(sse_stream(|emitter| async move {
        emitter
            .json("#widget", &serde_json::json!({"value": 7}))
            .await
    }))
    .await;
    assert!(body.contains("patch"), "event name missing:\n{body}");
    assert!(body.contains("#widget"), "target missing:\n{body}");
    assert!(body.contains("\"value\""), "data key missing:\n{body}");
}

// ── mutation_id ───────────────────────────────────────────────

#[tokio::test]
async fn sse_patch_with_mutation_id_appears_in_body() {
    let body = collect_sse_body(sse_stream(|emitter| async move {
        emitter
            .send(
                SilcrowEvent::patch(serde_json::json!({"n": 1}), "#a")
                    .with_mutation_id("mut-42"),
            )
            .await
    }))
    .await;
    assert!(body.contains("mut-42"), "mutation_id missing:\n{body}");
}

#[tokio::test]
async fn sse_patch_without_mutation_id_omits_key() {
    let body = collect_sse_body(sse_stream(|emitter| async move {
        emitter
            .send(SilcrowEvent::patch(serde_json::json!({"n": 1}), "#a"))
            .await
    }))
    .await;
    assert!(
        !body.contains("mutation_id"),
        "mutation_id key should be absent:\n{body}"
    );
}

// ── EmitError display ─────────────────────────────────────────

#[test]
fn emit_error_disconnected_display() {
    assert_eq!(
        EmitError::Disconnected.to_string(),
        "SSE client disconnected"
    );
}

#[test]
fn emit_error_serialize_display() {
    let err = EmitError::Serialize("bad payload".to_owned());
    assert!(err.to_string().contains("bad payload"));
}
