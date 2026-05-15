use super::model::{BakedSlot, BakedSlotKind};

/// Inject baked JSON values into slot anchors in a shell HTML string.
///
/// Finds `data-pilcrow-slot="name"` elements in `shell`, reads the corresponding value
/// from `json`, and replaces the element's inner content. Text slots are HTML-escaped;
/// TrustedHtml slots are injected verbatim.
///
/// Slots with no matching key in `json` are left with their existing content (the initial
/// value rendered into the shell by Askama is preserved).
pub fn inject_slots(shell: &str, json: &serde_json::Value, slots: &[BakedSlot]) -> String {
    let mut patches: Vec<(usize, usize, String)> = Vec::new();

    for slot in slots {
        let Some(json_val) = resolve_json_key(json, &slot.name) else {
            continue;
        };
        let raw = json_value_to_string(json_val);
        let replacement = match slot.kind {
            BakedSlotKind::Text => escape_html(&raw),
            BakedSlotKind::TrustedHtml => raw,
        };
        if let Some((start, end)) = find_slot_content_range(shell, &slot.name) {
            patches.push((start, end, replacement));
        }
    }

    if patches.is_empty() {
        return shell.to_string();
    }

    // Sort by position descending so byte offsets remain valid as we apply patches.
    patches.sort_by_key(|b| std::cmp::Reverse(b.0));

    let mut result = shell.to_string();
    for (start, end, replacement) in patches {
        result.replace_range(start..end, &replacement);
    }
    result
}

/// Resolve a dot-notation key path in a JSON object.
///
/// `"status"` → `json["status"]`
/// `"details.price"` → `json["details"]["price"]`
fn resolve_json_key<'a>(json: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let mut current = json;
    for segment in key.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

/// Find the byte range of the inner content of the first element with `data-pilcrow-slot="name"`.
///
/// Returns `(content_start, content_end)` — the range to replace with the new value.
/// The element tag and attributes are left intact; only the inner content changes.
fn find_slot_content_range(html: &str, name: &str) -> Option<(usize, usize)> {
    let attr = format!("data-pilcrow-slot=\"{}\"", name);
    let attr_pos = html.find(&attr)?;

    // Find the `>` that closes the opening tag.
    let tag_close = html[attr_pos..].find('>')?;
    let content_start = attr_pos + tag_close + 1;

    // Find `</` that starts the closing tag.
    let close_tag = html[content_start..].find("</")?;
    let content_end = content_start + close_tag;

    Some((content_start, content_end))
}

fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn text_slot(name: &str) -> BakedSlot {
        BakedSlot::text(name)
    }

    fn html_slot(name: &str) -> BakedSlot {
        BakedSlot::trusted_html(name)
    }

    #[test]
    fn injects_text_slot_from_json() {
        let shell = r#"<p>Status: <span data-pilcrow-slot="status">Loading</span></p>"#;
        let json = json!({ "status": "Resolved" });
        let result = inject_slots(shell, &json, &[text_slot("status")]);
        assert_eq!(
            result,
            r#"<p>Status: <span data-pilcrow-slot="status">Resolved</span></p>"#
        );
    }

    #[test]
    fn escapes_html_in_text_slots() {
        let shell = r#"<span data-pilcrow-slot="status">old</span>"#;
        let json = json!({ "status": "<script>alert('x')</script> & done" });
        let result = inject_slots(shell, &json, &[text_slot("status")]);
        assert!(result.contains("&lt;script&gt;"));
        assert!(result.contains("&amp; done"));
        assert!(!result.contains("<script>"));
    }

    #[test]
    fn injects_trusted_html_without_escaping() {
        let shell = r#"<div data-pilcrow-slot="body">placeholder</div>"#;
        let json = json!({ "body": "<strong>bold</strong>" });
        let result = inject_slots(shell, &json, &[html_slot("body")]);
        assert_eq!(
            result,
            r#"<div data-pilcrow-slot="body"><strong>bold</strong></div>"#
        );
    }

    #[test]
    fn missing_json_key_preserves_existing_content() {
        let shell = r#"<span data-pilcrow-slot="status">Initial</span>"#;
        let json = json!({ "other": "value" });
        let result = inject_slots(shell, &json, &[text_slot("status")]);
        assert_eq!(result, shell);
    }

    #[test]
    fn injects_multiple_slots() {
        let shell =
            r#"<span data-pilcrow-slot="a">old-a</span><span data-pilcrow-slot="b">old-b</span>"#;
        let json = json!({ "a": "new-a", "b": "new-b" });
        let result = inject_slots(shell, &json, &[text_slot("a"), text_slot("b")]);
        assert!(result.contains(">new-a<"));
        assert!(result.contains(">new-b<"));
    }

    #[test]
    fn resolves_dot_notation_key() {
        let json = json!({ "details": { "price": "9.99" } });
        let val = resolve_json_key(&json, "details.price").unwrap();
        assert_eq!(val, "9.99");
    }

    #[test]
    fn unknown_slot_anchor_leaves_html_unchanged() {
        let shell = r#"<p>No slots here</p>"#;
        let json = json!({ "status": "value" });
        let result = inject_slots(shell, &json, &[text_slot("status")]);
        assert_eq!(result, shell);
    }

    #[test]
    fn numeric_json_value_stringified() {
        let shell = r#"<span data-pilcrow-slot="count">0</span>"#;
        let json = json!({ "count": 42 });
        let result = inject_slots(shell, &json, &[text_slot("count")]);
        assert!(result.contains(">42<"));
    }
}
