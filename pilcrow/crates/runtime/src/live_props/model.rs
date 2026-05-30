use crate::deferred::DependencyKey;
use serde::{Deserialize, Serialize};

// ── LiveProp<T> ──────────────────────────────────────────────────────────────

/// A field value that participates in Pilcrow's live-props cache.
///
/// Returned from route handlers. The `#[pilcrow::handler(live)]` macro extracts
/// these fields and writes them to `pilcrow_cache` after every render.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveProp<T> {
    pub value: T,
    pub depends_on: Vec<DependencyKey>,
    pub patch_debounce: Option<u32>,
}

impl<T: Serialize + Clone> LiveProp<T> {
    pub fn new(value: T, depends_on: Vec<DependencyKey>) -> Self {
        Self {
            value,
            depends_on,
            patch_debounce: None,
        }
    }

    pub fn patch_debounce(mut self, seconds: u32) -> Self {
        self.patch_debounce = Some(seconds);
        self
    }

    pub fn to_field_data(&self, field_name: impl Into<String>) -> LiveFieldData {
        LiveFieldData {
            field_name: field_name.into(),
            json_value: serde_json::to_value(&self.value).unwrap_or(serde_json::Value::Null),
            depends_on: self.depends_on.clone(),
            patch_debounce: self.patch_debounce,
        }
    }
}

// ── LiveFieldData ─────────────────────────────────────────────────────────────

/// Extracted metadata for a single live field. Produced by `LivePropExtract::live_fields()`.
#[derive(Debug, Clone)]
pub struct LiveFieldData {
    pub field_name: String,
    pub json_value: serde_json::Value,
    pub depends_on: Vec<DependencyKey>,
    pub patch_debounce: Option<u32>,
}

// ── LivePropExtract ──────────────────────────────────────────────────────────

/// Implemented by structs that contain `LiveProp<T>` fields (via `#[derive(PilcrowProps)]`).
///
/// The `#[pilcrow::handler(live)]` macro calls this after the handler returns to extract
/// and persist live field data.
pub trait LivePropExtract {
    fn live_fields(&self) -> Vec<LiveFieldData>;
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_props_new_stores_value_and_deps() {
        let dep = DependencyKey::new("tickets:id=123");
        let lp = LiveProp::new("Open".to_string(), vec![dep.clone()]);
        assert_eq!(lp.value, "Open");
        assert_eq!(lp.depends_on, vec![dep]);
        assert!(lp.patch_debounce.is_none());
    }

    #[test]
    fn live_props_builder_sets_patch_debounce() {
        let lp = LiveProp::new("Open".to_string(), vec![]).patch_debounce(30);
        assert_eq!(lp.patch_debounce, Some(30));
    }

    #[test]
    fn live_field_data_from_live_props() {
        let dep = DependencyKey::new("tickets:id=123");
        let lp = LiveProp::new("Open".to_string(), vec![dep.clone()]).patch_debounce(5);
        let field = lp.to_field_data("status");
        assert_eq!(field.field_name, "status");
        assert_eq!(field.json_value, serde_json::json!("Open"));
        assert_eq!(field.depends_on, vec![dep]);
        assert_eq!(field.patch_debounce, Some(5));
    }

    #[test]
    fn multiple_deps_stored() {
        let deps = vec![
            DependencyKey::new("tickets:id=1"),
            DependencyKey::new("tickets:id=2"),
        ];
        let lp = LiveProp::new(42u32, deps.clone());
        let field = lp.to_field_data("count");
        assert_eq!(field.depends_on.len(), 2);
        assert_eq!(field.json_value, serde_json::json!(42));
    }
}
