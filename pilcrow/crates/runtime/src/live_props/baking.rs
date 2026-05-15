/// Inject live slot values into a shell HTML string.
///
/// Finds `data-pilcrow-live-field="name"` elements and replaces their inner
/// content with the matching value from `slots`. Values are always HTML-escaped.
/// Elements with no matching slot entry are left unchanged.
pub fn inject_live_slots(shell: &str, slots: &[(String, serde_json::Value)]) -> String {
    let mut patches: Vec<(usize, usize, String)> = Vec::new();

    for (slot_name, json_val) in slots {
        let raw = match json_val {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
        let escaped = escape_html(&raw);
        if let Some((start, end)) = find_live_field_content_range(shell, slot_name) {
            patches.push((start, end, escaped));
        }
    }

    if patches.is_empty() {
        return shell.to_string();
    }

    // Apply patches right-to-left so earlier byte offsets remain valid.
    patches.sort_by(|a, b| b.0.cmp(&a.0));
    let mut result = shell.to_string();
    for (start, end, replacement) in patches {
        result.replace_range(start..end, &replacement);
    }
    result
}

fn find_live_field_content_range(html: &str, name: &str) -> Option<(usize, usize)> {
    let attr = format!("data-pilcrow-live-field=\"{}\"", name);
    let attr_pos = html.find(&attr)?;
    let tag_close = html[attr_pos..].find('>')?;
    let content_start = attr_pos + tag_close + 1;
    let close_tag = html[content_start..].find("</")?;
    let content_end = content_start + close_tag;
    Some((content_start, content_end))
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_single_slot() {
        let shell = r#"<span data-pilcrow-live-field="status">Loading</span>"#;
        let slots = vec![("status".to_string(), serde_json::json!("Resolved"))];
        let result = inject_live_slots(shell, &slots);
        assert_eq!(
            result,
            r#"<span data-pilcrow-live-field="status">Resolved</span>"#
        );
    }

    #[test]
    fn escapes_html_in_value() {
        let shell = r#"<span data-pilcrow-live-field="title">old</span>"#;
        let slots = vec![(
            "title".to_string(),
            serde_json::json!("<script>alert(1)</script>"),
        )];
        let result = inject_live_slots(shell, &slots);
        assert!(result.contains("&lt;script&gt;"));
        assert!(!result.contains("<script>"));
    }

    #[test]
    fn missing_slot_leaves_content_unchanged() {
        let shell = r#"<span data-pilcrow-live-field="status">Default</span>"#;
        let slots: Vec<(String, serde_json::Value)> = vec![];
        let result = inject_live_slots(shell, &slots);
        assert_eq!(result, shell);
    }

    #[test]
    fn replaces_multiple_slots_in_one_pass() {
        let shell = r#"<span data-pilcrow-live-field="a">x</span><span data-pilcrow-live-field="b">y</span>"#;
        let slots = vec![
            ("a".to_string(), serde_json::json!("AAA")),
            ("b".to_string(), serde_json::json!("BBB")),
        ];
        let result = inject_live_slots(shell, &slots);
        assert!(result.contains(">AAA<"));
        assert!(result.contains(">BBB<"));
    }

    #[test]
    fn null_value_clears_slot_content() {
        let shell = r#"<span data-pilcrow-live-field="count">99</span>"#;
        let slots = vec![("count".to_string(), serde_json::Value::Null)];
        let result = inject_live_slots(shell, &slots);
        assert_eq!(result, r#"<span data-pilcrow-live-field="count"></span>"#);
    }
}
