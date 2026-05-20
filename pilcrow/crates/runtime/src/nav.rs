/// Extract a PS navigation fragment from a fully-rendered page HTML string.
///
/// Returns `<template data-ps-head>…</template>\n<div data-ps-slot="…">…</div>`.
/// Falls back to the full HTML if the markers are not found.
pub fn extract_ps_fragment(html: &str, slot_pattern: &str) -> String {
    let head = find_head_template(html).unwrap_or("");
    match find_slot_div(html, slot_pattern) {
        Some(slot) => format!("{head}\n{slot}"),
        None => html.to_owned(),
    }
}

fn find_head_template(html: &str) -> Option<&str> {
    const OPEN: &str = "<template data-ps-head>";
    const CLOSE: &str = "</template>";
    let start = html.find(OPEN)?;
    let after = start + OPEN.len();
    let end_off = html[after..].find(CLOSE)?;
    Some(&html[start..after + end_off + CLOSE.len()])
}

fn find_slot_div<'a>(html: &'a str, slot_pattern: &str) -> Option<&'a str> {
    let open_tag = format!("<div data-ps-slot=\"{slot_pattern}\">");
    let start = html.find(&open_tag)?;
    let after = start + open_tag.len();
    let mut depth = 1usize;
    let mut i = 0;
    let inner = &html[after..];
    while i < inner.len() {
        if inner[i..].starts_with("<div") {
            let nc = inner[i + 4..].chars().next();
            if nc.map_or(false, |c| c == '>' || c == ' ' || c == '\n' || c == '\t' || c == '/') {
                depth += 1;
            }
            i += 4;
        } else if inner[i..].starts_with("</div>") {
            depth -= 1;
            if depth == 0 {
                return Some(&html[start..after + i + 6]);
            }
            i += 6;
        } else {
            i += inner[i..].chars().next().map_or(1, |c| c.len_utf8());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_head_and_slot() {
        let html = r#"<html><body>
<template data-ps-head><title>Test</title></template>
<div data-ps-slot="/tickets/:id"><p>content</p></div>
</body></html>"#;
        let result = extract_ps_fragment(html, "/tickets/:id");
        assert!(result.contains("<template data-ps-head>"));
        assert!(result.contains("<title>Test</title>"));
        assert!(result.contains(r#"<div data-ps-slot="/tickets/:id">"#));
        assert!(result.contains("<p>content</p>"));
        assert!(!result.contains("<html>"));
    }

    #[test]
    fn fallback_to_full_html_when_slot_missing() {
        let html = "<html><body><p>no slot</p></body></html>";
        let result = extract_ps_fragment(html, "/tickets/:id");
        assert_eq!(result, html);
    }

    #[test]
    fn nested_divs_are_handled() {
        let html = r#"<div data-ps-slot="/a"><div><div>deep</div></div></div>"#;
        let result = extract_ps_fragment(html, "/a");
        assert!(result.contains(r#"<div data-ps-slot="/a">"#));
        assert!(result.contains("deep"));
    }
}
