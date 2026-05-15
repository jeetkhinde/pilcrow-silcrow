/// Inject FSR slot values into a shell HTML string.
///
/// Finds `s-live="name"` elements and replaces their inner content with the
/// matching value from `slots`. Values are always HTML-escaped. Elements with
/// no matching slot entry are left unchanged.
pub fn inject_fsr_slots(shell: &str, slots: &[(String, serde_json::Value)]) -> String {
    let mut patches: Vec<(usize, usize, String)> = Vec::new();

    for (slot_name, json_val) in slots {
        let raw = match json_val {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
        let escaped = escape_html(&raw);
        if let Some((start, end)) = find_s_live_content_range(shell, slot_name) {
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

fn find_s_live_content_range(html: &str, name: &str) -> Option<(usize, usize)> {
    let attr = format!("s-live=\"{}\"", name);
    let attr_pos = html.find(&attr)?;
    let tag_close = html[attr_pos..].find('>')?;
    let content_start = attr_pos + tag_close + 1;
    let close_tag = html[content_start..].find("</")?;
    let content_end = content_start + close_tag;
    Some((content_start, content_end))
}

/// Scan HTML for all `s-live="..."` attribute values and return the slot names.
///
/// Duplicate names are deduplicated; empty names are ignored.
pub fn find_s_live_slots(html: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut remaining = html;
    while let Some(pos) = remaining.find("s-live=\"") {
        remaining = &remaining[pos + 8..];
        if let Some(end) = remaining.find('"') {
            let name = remaining[..end].to_string();
            if !name.is_empty() && !names.contains(&name) {
                names.push(name);
            }
            remaining = &remaining[end + 1..];
        }
    }
    names
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

    // ── inject_fsr_slots ─────────────────────────────────────────────────────

    #[test]
    fn replaces_single_slot() {
        let shell = r#"<span s-live="ticket_status">Loading</span>"#;
        let slots = vec![("ticket_status".to_string(), serde_json::json!("Resolved"))];
        let result = inject_fsr_slots(shell, &slots);
        assert_eq!(result, r#"<span s-live="ticket_status">Resolved</span>"#);
    }

    #[test]
    fn escapes_html_in_value() {
        let shell = r#"<span s-live="title">old</span>"#;
        let slots = vec![(
            "title".to_string(),
            serde_json::json!("<script>alert(1)</script>"),
        )];
        let result = inject_fsr_slots(shell, &slots);
        assert!(result.contains("&lt;script&gt;"));
        assert!(!result.contains("<script>"));
    }

    #[test]
    fn missing_slot_leaves_content_unchanged() {
        let shell = r#"<span s-live="status">Default</span>"#;
        let slots: Vec<(String, serde_json::Value)> = vec![];
        let result = inject_fsr_slots(shell, &slots);
        assert_eq!(result, shell);
    }

    #[test]
    fn replaces_multiple_slots_in_one_pass() {
        let shell = r#"<span s-live="a">x</span><span s-live="b">y</span>"#;
        let slots = vec![
            ("a".to_string(), serde_json::json!("AAA")),
            ("b".to_string(), serde_json::json!("BBB")),
        ];
        let result = inject_fsr_slots(shell, &slots);
        assert!(result.contains(">AAA<"));
        assert!(result.contains(">BBB<"));
    }

    #[test]
    fn null_value_clears_slot_content() {
        let shell = r#"<span s-live="count">99</span>"#;
        let slots = vec![("count".to_string(), serde_json::Value::Null)];
        let result = inject_fsr_slots(shell, &slots);
        assert_eq!(result, r#"<span s-live="count"></span>"#);
    }

    #[test]
    fn numeric_json_value_serialised_as_string() {
        let shell = r#"<span s-live="price">0</span>"#;
        let slots = vec![("price".to_string(), serde_json::json!(42))];
        let result = inject_fsr_slots(shell, &slots);
        assert_eq!(result, r#"<span s-live="price">42</span>"#);
    }

    #[test]
    fn unrelated_slot_name_not_matched() {
        let shell = r#"<span s-live="status">Open</span>"#;
        let slots = vec![("priority".to_string(), serde_json::json!("High"))];
        let result = inject_fsr_slots(shell, &slots);
        // status element unchanged, priority has no element so no op
        assert_eq!(result, shell);
    }

    // ── find_s_live_slots ────────────────────────────────────────────────────

    #[test]
    fn finds_single_slot_name() {
        let html = r#"<span s-live="ticket_status">Loading</span>"#;
        let names = find_s_live_slots(html);
        assert_eq!(names, vec!["ticket_status"]);
    }

    #[test]
    fn finds_multiple_distinct_slot_names() {
        let html = r#"<span s-live="a">x</span><span s-live="b">y</span><span s-live="c">z</span>"#;
        let names = find_s_live_slots(html);
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn deduplicates_repeated_slot_names() {
        let html = r#"<span s-live="status">x</span><div s-live="status">y</div>"#;
        let names = find_s_live_slots(html);
        assert_eq!(names, vec!["status"]);
    }

    #[test]
    fn returns_empty_vec_when_no_slots_present() {
        let html = r#"<span class="foo">hello</span>"#;
        let names = find_s_live_slots(html);
        assert!(names.is_empty());
    }

    #[test]
    fn ignores_data_pilcrow_live_field_attributes() {
        // The two attribute namespaces must not interfere.
        let html =
            r#"<span data-pilcrow-live-field="status">x</span><span s-live="count">0</span>"#;
        let names = find_s_live_slots(html);
        assert_eq!(names, vec!["count"]);
    }
}
