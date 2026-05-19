/// Tests for `WsEvent` serialization and `WsRecvError` display.
use runtime::{WsEvent, ws::ws::WsRecvError};

// ── WsEvent serialization ─────────────────────────────────────

#[test]
fn ws_event_patch_serializes() {
    let evt = WsEvent::patch(serde_json::json!({"count": 3}), "#app");
    let v: serde_json::Value = serde_json::to_value(&evt).unwrap();
    assert_eq!(v["type"], "patch");
    assert_eq!(v["target"], "#app");
    assert_eq!(v["data"]["count"], 3);
}

#[test]
fn ws_event_html_serializes() {
    let evt = WsEvent::html("<p>hi</p>", "#content");
    let v: serde_json::Value = serde_json::to_value(&evt).unwrap();
    assert_eq!(v["type"], "html");
    assert_eq!(v["target"], "#content");
    assert_eq!(v["markup"], "<p>hi</p>");
}

#[test]
fn ws_event_invalidate_serializes() {
    let evt = WsEvent::invalidate("#list");
    let v: serde_json::Value = serde_json::to_value(&evt).unwrap();
    assert_eq!(v["type"], "invalidate");
    assert_eq!(v["target"], "#list");
}

#[test]
fn ws_event_navigate_serializes() {
    let evt = WsEvent::navigate("/home");
    let v: serde_json::Value = serde_json::to_value(&evt).unwrap();
    assert_eq!(v["type"], "navigate");
    assert_eq!(v["path"], "/home");
}

#[test]
fn ws_event_custom_serializes() {
    let evt = WsEvent::custom("ping", serde_json::json!({"seq": 1}));
    let v: serde_json::Value = serde_json::to_value(&evt).unwrap();
    assert_eq!(v["type"], "custom");
    assert_eq!(v["event"], "ping");
    assert_eq!(v["data"]["seq"], 1);
}

// ── round-trip ────────────────────────────────────────────────

#[test]
fn ws_event_patch_round_trips() {
    let original = WsEvent::patch(serde_json::json!({"x": 42}), "#box");
    let json = serde_json::to_string(&original).unwrap();
    let decoded: WsEvent = serde_json::from_str(&json).unwrap();
    match decoded {
        WsEvent::Patch { target, data, .. } => {
            assert_eq!(target, "#box");
            assert_eq!(data["x"], 42);
        }
        other => panic!("expected Patch, got {other:?}"),
    }
}

#[test]
fn ws_event_navigate_round_trips() {
    let original = WsEvent::navigate("/checkout");
    let json = serde_json::to_string(&original).unwrap();
    let decoded: WsEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded, WsEvent::Navigate { path } if path == "/checkout"));
}

// ── mutation_id ───────────────────────────────────────────────

#[test]
fn ws_event_patch_with_mutation_id_serializes() {
    let evt = WsEvent::patch(serde_json::json!({"n": 1}), "#a").with_mutation_id("mut-7");
    let v: serde_json::Value = serde_json::to_value(&evt).unwrap();
    assert_eq!(v["mutation_id"], "mut-7");
}

#[test]
fn ws_event_patch_without_mutation_id_omits_key() {
    let evt = WsEvent::patch(serde_json::json!({"n": 1}), "#a");
    let v: serde_json::Value = serde_json::to_value(&evt).unwrap();
    assert!(
        v.get("mutation_id").is_none(),
        "mutation_id should be absent"
    );
}

// ── WsRecvError display ───────────────────────────────────────

#[test]
fn ws_recv_error_closed_display() {
    assert_eq!(WsRecvError::Closed.to_string(), "WsRecvError::Closed");
}

#[test]
fn ws_recv_error_non_text_display() {
    assert_eq!(WsRecvError::NonText.to_string(), "WsRecvError::NonText");
}

#[test]
fn ws_recv_error_deserialize_display() {
    let err: serde_json::Error = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    let recv_err = WsRecvError::Deserialize(err);
    assert!(recv_err.to_string().starts_with("WsRecvError::Deserialize"));
}
