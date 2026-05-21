// Integration test for #[derive(PilcrowListRow)].
//
// This file compiles as a separate crate that depends on pilcrow-web, so
// ::pilcrow_web::live::ListRow must resolve — this is the exact consumer
// scenario the P1A fix targets.
#![cfg(feature = "live-props")]

use pilcrow_web::live::ListRow as _;
use pilcrow_web::PilcrowListRow;

#[derive(PilcrowListRow)]
struct TicketRow {
    #[pilcrow(key)]
    id: i64,
    #[pilcrow(live)]
    status: String,
    title: String,
}

#[test]
fn key_field_returns_string_of_id() {
    let row = TicketRow { id: 42, status: "open".to_string(), title: "Fix bug".to_string() };
    assert_eq!(row.pilcrow_key(), "42");
}

#[test]
fn live_fields_includes_only_live_annotated_fields() {
    let row = TicketRow { id: 1, status: "closed".to_string(), title: "ignored".to_string() };
    let fields = row.pilcrow_live_fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].0, "status");
    assert_eq!(fields[0].1, serde_json::json!("closed"));
}

#[test]
fn live_fields_excludes_key_and_static_fields() {
    let row = TicketRow { id: 99, status: "open".to_string(), title: "not live".to_string() };
    let fields = row.pilcrow_live_fields();
    assert!(fields.iter().all(|(name, _)| *name != "id" && *name != "title"));
}
