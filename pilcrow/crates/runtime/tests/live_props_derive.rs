#![cfg(feature = "live-props")]

use pilcrow_macros::PilcrowProps;
use runtime::baked_pages::DependencyKey;
use runtime::live_props::{LiveProp, LivePropExtract};

#[derive(PilcrowProps)]
struct TicketProps {
    #[patch_debounce(30)]
    pub status: LiveProp<String>,
    pub title: String, // non-LiveProp field — must be ignored
}

#[test]
fn derive_extracts_only_live_props_fields() {
    let props = TicketProps {
        status: LiveProp::new(
            "Open".to_string(),
            vec![DependencyKey::new("tickets:id=123")],
        ),
        title: "My bug".to_string(),
    };
    let fields = props.live_fields();
    assert_eq!(
        fields.len(),
        1,
        "only LiveProp<T> fields should be extracted"
    );
    assert_eq!(fields[0].field_name, "status");
    assert_eq!(fields[0].json_value, serde_json::json!("Open"));
    assert_eq!(
        fields[0].depends_on,
        vec![DependencyKey::new("tickets:id=123")]
    );
    assert_eq!(fields[0].patch_debounce, Some(30));
}

#[derive(PilcrowProps)]
struct MultiFieldProps {
    pub status: LiveProp<String>,
    pub priority: LiveProp<u32>,
    pub not_live: bool,
}

#[test]
fn derive_handles_multiple_live_props_fields() {
    let props = MultiFieldProps {
        status: LiveProp::new("Open".to_string(), vec![]),
        priority: LiveProp::new(1u32, vec![]),
        not_live: true,
    };
    let fields = props.live_fields();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].field_name, "status");
    assert_eq!(fields[1].field_name, "priority");
    assert_eq!(fields[1].json_value, serde_json::json!(1));
}

#[derive(PilcrowProps)]
struct NoBakeProps {
    pub count: LiveProp<i64>,
}

#[test]
fn derive_works_without_field_attributes() {
    let props = NoBakeProps {
        count: LiveProp::new(42i64, vec![]),
    };
    let fields = props.live_fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].field_name, "count");
    assert_eq!(fields[0].json_value, serde_json::json!(42));
    assert!(fields[0].patch_debounce.is_none());
}

// The #[column] attribute renames the field_name used for HTML slot injection.
// This keeps the Rust field name (ticket_status) decoupled from the HTML slot name (status).
#[derive(PilcrowProps)]
struct RenamedColumnProps {
    #[column("status")]
    pub ticket_status: LiveProp<String>,
    pub priority: LiveProp<String>,
}

#[test]
fn column_attribute_overrides_field_name_for_html_slot() {
    let props = RenamedColumnProps {
        ticket_status: LiveProp::new("Open".to_string(), vec![]),
        priority: LiveProp::new("High".to_string(), vec![]),
    };
    let fields = props.live_fields();
    assert_eq!(fields.len(), 2);
    // Renamed field: field_name reflects the column override (used as HTML slot name).
    assert_eq!(fields[0].field_name, "status");
    assert_eq!(fields[0].json_value, serde_json::json!("Open"));
    // Non-renamed field: field_name is the Rust field name.
    assert_eq!(fields[1].field_name, "priority");
    assert_eq!(fields[1].json_value, serde_json::json!("High"));
}
