use std::fmt;

/// Split result for a Pilcrow `.html` file that uses fenced Rust + template sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlModuleParts {
    pub rust: String,
    pub template: String,
}

/// Parse failures for `.html` module splitting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlModuleParseError {
    MissingFence,
    EmptyTemplate,
}

impl fmt::Display for HtmlModuleParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFence => {
                write!(
                    f,
                    "expected `---` fenced file format with Rust block and template block"
                )
            }
            Self::EmptyTemplate => write!(f, "template section after second fence is empty"),
        }
    }
}

impl std::error::Error for HtmlModuleParseError {}

/// Splits a `.html` source string into Rust frontmatter and template body.
///
/// Expected format:
///
/// ```text
/// ---
/// // Rust code...
/// ---
/// <h1>Template</h1>
/// ```
pub fn split_html_module(input: &str) -> Result<HtmlModuleParts, HtmlModuleParseError> {
    // A file with no leading `---` fence is treated as pure template body with
    // empty Rust frontmatter. This lets purely-static pages and components omit
    // the ceremony of declaring an empty `pub struct Props {}`.
    if !input.trim_start().starts_with("---") {
        let template = input.trim();
        if template.is_empty() {
            return Err(HtmlModuleParseError::EmptyTemplate);
        }
        return Ok(HtmlModuleParts {
            rust: String::new(),
            template: template.to_string(),
        });
    }

    let mut parts = input.splitn(3, "---");
    let leading = parts.next().unwrap_or_default();
    let rust = parts.next().ok_or(HtmlModuleParseError::MissingFence)?;
    let template = parts.next().ok_or(HtmlModuleParseError::MissingFence)?;

    if !leading.trim().is_empty() {
        return Err(HtmlModuleParseError::MissingFence);
    }

    let template = template.trim();
    if template.is_empty() {
        return Err(HtmlModuleParseError::EmptyTemplate);
    }

    Ok(HtmlModuleParts {
        rust: rust.trim().to_string(),
        template: template.to_string(),
    })
}

/// Splits and transpiles a Pilcrow `.html` module.
///
/// This performs:
/// 1. `---` fence splitting
/// 2. component tag transpilation in the template section
/// 3. form progressive-enhancement injection
#[allow(dead_code)]
pub fn transpile_html_module(input: &str) -> Result<HtmlModuleParts, HtmlModuleParseError> {
    let mut parts = split_html_module(input)?;
    parts.template = transpile_component_tags(&parts.template);
    parts.template = inject_form_method_attrs(&parts.template);
    Ok(parts)
}

/// Rewrites `{{ field }}` interpolations for `LiveProp<T>` fields into
/// `<span data-pilcrow-live-field="field">{{ field }}</span>` so the live-patch
/// head shim can update them via SSE without relying on Silcrow's `:text` binding.
///
/// Only fields whose names appear in `live_fields` are wrapped. Fields with
/// `LiveTarget::Store` (atom-only, no DOM binding) should not appear in this
/// list — only `Dom` and `DomAndStore` fields need the span.
pub fn inject_live_text_spans(template: &str, live_fields: &[String]) -> String {
    if live_fields.is_empty() {
        return template.to_string();
    }
    let mut out = template.to_string();
    for field in live_fields {
        let expr = format!("{{{{ {field} }}}}");
        let span = format!("<span data-pilcrow-live-field=\"{field}\">{{{{ {field} }}}}</span>");
        out = out.replace(&expr, &span);
        let expr_ns = format!("{{{{{field}}}}}");
        if expr_ns != expr {
            let span_ns =
                format!("<span data-pilcrow-live-field=\"{field}\">{{{{{field}}}}}</span>");
            out = out.replace(&expr_ns, &span_ns);
        }
    }
    out
}

/// Rewrites `{{ field }}` interpolations for `AsyncValue<T>` fields into a
/// compact Pilcrow-owned target for deferred scalar patches.
pub fn inject_async_value_text_spans(template: &str, async_fields: &[String]) -> String {
    if async_fields.is_empty() {
        return template.to_string();
    }

    fn async_value_attr(field: &str) -> String {
        format!("data-pilcrow-async-value=\"{field}\"")
    }

    fn already_bound_before(input: &str, idx: usize, field: &str) -> bool {
        let Some(open_end) = input[..idx].rfind('>') else {
            return false;
        };
        let Some(open_start) = input[..open_end].rfind('<') else {
            return false;
        };
        let tag = &input[open_start..=open_end];
        tag.contains(&format!("data-pilcrow-async-value=\"{field}\""))
            || tag.contains(&format!("data-pilcrow-async-value='{field}'"))
    }

    fn can_augment_parent(input: &str, idx: usize, expr_len: usize) -> bool {
        let Some(open_end) = input[..idx].rfind('>') else {
            return false;
        };
        if !input[open_end + 1..idx].trim().is_empty() {
            return false;
        }
        let after_expr = idx + expr_len;
        let Some(close_start_rel) = input[after_expr..].find("</") else {
            return false;
        };
        input[after_expr..after_expr + close_start_rel]
            .trim()
            .is_empty()
    }

    fn augment_last_open_tag(out: &mut String, field: &str) -> bool {
        let Some(open_end) = out.rfind('>') else {
            return false;
        };
        let Some(open_start) = out[..open_end].rfind('<') else {
            return false;
        };
        let tag = &out[open_start..=open_end];
        if tag.starts_with("</") || tag.contains(&async_value_attr(field)) {
            return false;
        }

        let attrs = format!(" {}", async_value_attr(field));
        out.insert_str(open_end, &attrs);
        true
    }

    #[allow(clippy::if_same_then_else)]
    fn replace_expr(input: String, expr: &str, replacement: &str, field: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut cursor = 0;
        while let Some(rel) = input[cursor..].find(expr) {
            let idx = cursor + rel;
            out.push_str(&input[cursor..idx]);
            if already_bound_before(&input, idx, field) {
                out.push_str(expr);
            } else if can_augment_parent(&input, idx, expr.len())
                && augment_last_open_tag(&mut out, field)
            {
                out.push_str(expr);
            } else {
                out.push_str(replacement);
            }
            cursor = idx + expr.len();
        }
        out.push_str(&input[cursor..]);
        out
    }

    let mut out = template.to_string();
    for field in async_fields {
        let expr = format!("{{{{ {field} }}}}");
        let span = format!("<span data-pilcrow-async-value=\"{field}\">{expr}</span>");
        out = replace_expr(out, &expr, &span, field);

        let expr_ns = format!("{{{{{field}}}}}");
        if expr_ns != expr {
            let span_ns = format!("<span data-pilcrow-async-value=\"{field}\">{expr_ns}</span>");
            out = replace_expr(out, &expr_ns, &span_ns, field);
        }
    }
    out
}

// ── HTTP Verb Attributes ──────────────────────────────────────

/// Maps silcrow verb attributes to their native HTTP method strings.
/// HTML forms only support GET/POST natively; all mutation verbs map to POST.
const VERB_ATTRS: &[(&str, &str)] = &[
    ("s-post", "post"),
    ("s-put", "post"),
    ("s-patch", "post"),
    ("s-delete", "post"),
];

/// Injects `method` and `action` attributes into `<form>` elements that carry
/// a silcrow verb attribute (`s-post`, `s-put`, `s-patch`, `s-delete`).
///
/// This enables progressive enhancement: a form with `s-post="?action=create"`
/// works as a native HTML form submit when JS is unavailable, and silcrow.js
/// intercepts it when JS is present.
///
/// Rules:
/// - `s-post`   → `method="post"`
/// - `s-put|s-patch|s-delete` → `method="post"` (HTML only supports GET/POST natively)
/// - If `method` already exists on the element: not overwritten.
/// - If `action` already exists on the element: not overwritten.
/// - `s-get` on forms is left alone — GET is already the HTML default.
pub fn inject_form_method_attrs(template: &str) -> String {
    let mut output = String::with_capacity(template.len() + 64);
    let mut i = 0;

    while i < template.len() {
        // Fast path: look for the literal substring "<form"
        if template[i..].starts_with("<form") {
            let after = i + 5;
            let next_char = template[after..].chars().next();
            // Must be followed by whitespace or '>' to be a real <form> tag
            if matches!(next_char, Some(c) if c.is_whitespace() || c == '>')
                && let Some((transformed, consumed)) = try_inject_form_tag(&template[i..])
            {
                output.push_str(&transformed);
                i += consumed;
                continue;
            }
        }

        let c = template[i..].chars().next().unwrap();
        output.push(c);
        i += c.len_utf8();
    }

    output
}

/// Try to transform a `<form ...>` opening tag by injecting progressive-enhancement
/// attributes. Returns `None` if the tag has no silcrow verb attribute (no-op).
fn try_inject_form_tag(input: &str) -> Option<(String, usize)> {
    debug_assert!(input.starts_with("<form"));

    // Collect the raw text of the opening tag's attribute section (between "<form" and ">").
    let mut idx = 5; // skip "<form"
    let mut raw_attrs = String::new();
    let mut quote: Option<char> = None;
    let mut brace_depth: usize = 0;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            raw_attrs.push(c);
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => {
                quote = Some(c);
                raw_attrs.push(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                raw_attrs.push(c);
                idx += c_len;
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                raw_attrs.push(c);
                idx += c_len;
            }
            '>' if brace_depth == 0 => {
                // Found the end of the opening tag.
                let tag_end = idx + 1; // include '>'

                // Does this form have a silcrow mutation verb attribute?
                let verb_match = VERB_ATTRS.iter().find_map(|&(attr, method)| {
                    html_attr_value(&raw_attrs, attr).map(|url| (method, url))
                });

                let (http_method, url) = verb_match?; // no verb → return None (no-op)

                let has_method = html_has_attr(&raw_attrs, "method");
                let has_action = html_has_attr(&raw_attrs, "action");

                if has_method && has_action {
                    return None; // nothing to inject
                }

                let mut inject = String::new();
                if !has_method {
                    inject.push_str(&format!(" method=\"{http_method}\""));
                }
                if !has_action {
                    let safe_url = url.replace('"', "&quot;");
                    inject.push_str(&format!(" action=\"{safe_url}\""));
                }

                let transformed = format!("<form{raw_attrs}{inject}>");
                return Some((transformed, tag_end));
            }
            _ => {
                raw_attrs.push(c);
                idx += c_len;
            }
        }
    }

    None
}

/// Extract the value of a named HTML attribute from a raw attribute string.
/// Handles `name="value"`, `name='value'`. Returns `None` if not found.
fn html_attr_value(attrs: &str, name: &str) -> Option<String> {
    let mut i = 0;
    while i < attrs.len() {
        i = skip_ws(attrs, i);
        if i >= attrs.len() {
            break;
        }

        let (attr_name, next) = scan_html_attr_name(attrs, i);
        i = skip_ws(attrs, next);

        if i < attrs.len() && attrs[i..].starts_with('=') {
            i += 1; // skip '='
            i = skip_ws(attrs, i);
            if let Some((val, end)) = scan_html_attr_val(attrs, i) {
                if attr_name == name {
                    return Some(val);
                }
                i = end;
            } else {
                i = next; // can't parse value; skip
            }
        } else {
            // Bare attribute (no value)
            if attr_name == name {
                return Some(String::new());
            }
        }
    }
    None
}

/// Return true if a named attribute is present in the raw attribute string.
fn html_has_attr(attrs: &str, name: &str) -> bool {
    html_attr_value(attrs, name).is_some()
}

/// Scan an HTML attribute name (letters, digits, hyphens, colons, underscores, dots).
/// Returns `(name, end_index)`.
fn scan_html_attr_name(src: &str, start: usize) -> (String, usize) {
    let mut idx = start;
    while idx < src.len() {
        let c = src[idx..].chars().next().unwrap_or('\0');
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.') {
            idx += c.len_utf8();
        } else {
            break;
        }
    }
    (src[start..idx].to_string(), idx)
}

/// Scan a quoted (`"..."` or `'...'`) or braced (`{...}`) HTML attribute value.
/// Returns `(value_text, end_index)`.
fn scan_html_attr_val(src: &str, start: usize) -> Option<(String, usize)> {
    let first = src[start..].chars().next()?;
    match first {
        '"' | '\'' => {
            let mut idx = start + 1;
            while idx < src.len() {
                let c = src[idx..].chars().next()?;
                if c == first {
                    let val = src[start + 1..idx].to_string();
                    return Some((val, idx + 1));
                }
                idx += c.len_utf8();
            }
            None
        }
        '{' => {
            let mut depth = 1usize;
            let mut idx = start + 1;
            while idx < src.len() {
                let c = src[idx..].chars().next()?;
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some((src[start + 1..idx].to_string(), idx + 1));
                        }
                    }
                    _ => {}
                }
                idx += c.len_utf8();
            }
            None
        }
        _ => None,
    }
}

/// Transpiles PascalCase component tags into Askama expressions.
///
/// Example:
/// `<Card title={item.title} />`
/// becomes
/// `{{ Card { title: item.title }|safe }}`
///
/// For paired component tags, inner content is captured into a synthetic
/// `children` field:
/// `<Layout title={title}>...</Layout>`
/// becomes
/// `{{ Layout { title: title, children: r#"..."# }|safe }}`
pub fn transpile_component_tags(template: &str) -> String {
    let mut output = String::with_capacity(template.len());
    let mut i = 0usize;

    while i < template.len() {
        let Some(ch) = template[i..].chars().next() else {
            break;
        };

        if let Some(consumed) = copy_html_comment(template, i, &mut output) {
            i += consumed;
            continue;
        }

        if ch == '<'
            && let Some((replacement, consumed)) = parse_component_tag(&template[i..])
        {
            output.push_str(&replacement);
            i += consumed;
            continue;
        }

        output.push(ch);
        i += ch.len_utf8();
    }

    output
}

fn parse_component_tag(input: &str) -> Option<(String, usize)> {
    let mut idx = 0usize;

    // Must start with "<"
    idx += consume_char(input, idx, '<')?;

    // Component name: PascalCase + [a-zA-Z0-9_]
    let first = input[idx..].chars().next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }

    let mut name_end = idx + first.len_utf8();
    while let Some(c) = input[name_end..].chars().next() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name_end += c.len_utf8();
        } else {
            break;
        }
    }

    let name = &input[idx..name_end];
    idx = name_end;

    // Parse opening tag until "/>" or ">", honoring nested braces and quoted strings.
    let attrs_start = idx;
    let mut brace_depth = 0usize;
    let mut quote: Option<char> = None;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => {
                quote = Some(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                idx += c_len;
            }
            '}' => {
                if brace_depth == 0 {
                    return None;
                }
                brace_depth -= 1;
                idx += c_len;
            }
            '/' if brace_depth == 0 && input[idx..].starts_with("/>") => {
                let attrs_src = &input[attrs_start..idx];
                let attrs = parse_attributes(attrs_src)?;
                let rendered = render_component_call(name, &attrs);
                return Some((rendered, idx + 2));
            }
            '>' => {
                let attrs_src = &input[attrs_start..idx];
                let attrs = parse_attributes(attrs_src)?;
                let open_end = idx + c_len;
                let (inner_len, close_len) =
                    find_matching_component_close(&input[open_end..], name)?;
                let inner = &input[open_end..open_end + inner_len];
                let consumed = open_end + inner_len + close_len;

                let inner_transpiled = transpile_component_tags(inner);
                let rendered =
                    render_component_call_with_children(name, &attrs, &inner_transpiled)?;
                return Some((rendered, consumed));
            }
            _ => {
                idx += c_len;
            }
        }
    }

    None
}

fn parse_attributes(src: &str) -> Option<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut idx = 0usize;

    while idx < src.len() {
        idx = skip_ws(src, idx);
        if idx >= src.len() {
            break;
        }

        let (name, next_idx) = parse_attr_name(src, idx)?;
        idx = skip_ws(src, next_idx);

        if idx >= src.len() || !src[idx..].starts_with('=') {
            // Bare attrs (e.g. disabled) become booleans.
            out.push((name, "true".to_string()));
            continue;
        }
        idx += 1; // '='
        idx = skip_ws(src, idx);

        let (expr, consumed_to) = parse_attr_value(src, idx)?;
        out.push((name, expr));
        idx = consumed_to;
    }

    Some(out)
}

fn parse_attr_name(src: &str, start: usize) -> Option<(String, usize)> {
    let first = src[start..].chars().next()?;
    if !is_rust_ident_start(first) {
        return None;
    }

    let mut idx = start + first.len_utf8();
    while let Some(c) = src[idx..].chars().next() {
        if is_rust_ident_continue(c) {
            idx += c.len_utf8();
        } else {
            break;
        }
    }

    Some((src[start..idx].to_string(), idx))
}

fn parse_attr_value(src: &str, start: usize) -> Option<(String, usize)> {
    let first = src[start..].chars().next()?;
    match first {
        '{' => parse_braced_expr(src, start),
        '"' | '\'' => parse_quoted_expr(src, start, first),
        _ => {
            // Unquoted token
            let mut idx = start;
            while let Some(c) = src[idx..].chars().next() {
                if c.is_whitespace() {
                    break;
                }
                idx += c.len_utf8();
            }
            Some((src[start..idx].to_string(), idx))
        }
    }
}

fn parse_braced_expr(src: &str, start: usize) -> Option<(String, usize)> {
    let mut idx = start + 1; // skip opening {
    let mut depth = 1usize;

    while idx < src.len() {
        let c = src[idx..].chars().next()?;
        let c_len = c.len_utf8();
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let expr = src[start + 1..idx].trim().to_string();
                    return Some((expr, idx + c_len));
                }
            }
            _ => {}
        }
        idx += c_len;
    }
    None
}

fn parse_quoted_expr(src: &str, start: usize, quote: char) -> Option<(String, usize)> {
    let mut idx = start + quote.len_utf8();
    while idx < src.len() {
        let c = src[idx..].chars().next()?;
        let c_len = c.len_utf8();
        if c == quote {
            let raw = &src[start + quote.len_utf8()..idx];
            let expr = askama_expr_or_string_literal(raw);
            return Some((expr, idx + c_len));
        }
        idx += c_len;
    }
    None
}

fn askama_expr_or_string_literal(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed
        .strip_prefix("{{")
        .and_then(|v| v.strip_suffix("}}"))
    {
        return inner.trim().to_string();
    }

    format!("{raw:?}")
}

fn render_component_call(name: &str, attrs: &[(String, String)]) -> String {
    if attrs.is_empty() {
        return format!("{{{{ {name} {{}}|safe }}}}");
    }

    let body = attrs
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join(", ");

    format!("{{{{ {name} {{ {body} }}|safe }}}}")
}

fn render_component_call_with_children(
    name: &str,
    attrs: &[(String, String)],
    inner: &str,
) -> Option<String> {
    if inner.trim().is_empty() {
        return Some(render_component_call(name, attrs));
    }

    if attrs.iter().any(|(k, _)| k == "children") {
        return None;
    }

    let mut all = attrs.to_vec();
    all.push(("children".to_string(), rust_raw_string_literal(inner)));
    Some(render_component_call(name, &all))
}

fn rust_raw_string_literal(value: &str) -> String {
    for hashes_count in 0..=32usize {
        let hashes = "#".repeat(hashes_count);
        let terminator = format!("\"{hashes}");
        if !value.contains(&terminator) {
            return format!("r{hashes}\"{value}\"{hashes}");
        }
    }
    format!("{value:?}")
}

fn find_matching_component_close(input: &str, name: &str) -> Option<(usize, usize)> {
    let mut idx = 0usize;
    let mut depth = 1usize;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        if c != '<' {
            idx += c.len_utf8();
            continue;
        }

        if let Some(consumed) = parse_named_close_tag(input, idx, name) {
            depth -= 1;
            if depth == 0 {
                return Some((idx, consumed));
            }
            idx += consumed;
            continue;
        }

        if let Some((consumed, self_closing)) = parse_named_open_tag(input, idx, name) {
            if !self_closing {
                depth += 1;
            }
            idx += consumed;
            continue;
        }

        idx += c.len_utf8();
    }

    None
}

fn parse_named_open_tag(input: &str, start: usize, name: &str) -> Option<(usize, bool)> {
    if !input[start..].starts_with('<') {
        return None;
    }

    let mut idx = start + 1;
    if input[idx..].starts_with('/') {
        return None;
    }
    if !input[idx..].starts_with(name) {
        return None;
    }
    idx += name.len();

    let boundary = input[idx..].chars().next()?;
    if !(boundary.is_whitespace() || boundary == '/' || boundary == '>') {
        return None;
    }

    let attrs_start = idx;
    let mut brace_depth = 0usize;
    let mut quote: Option<char> = None;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => {
                quote = Some(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                idx += c_len;
            }
            '}' => {
                if brace_depth == 0 {
                    return None;
                }
                brace_depth -= 1;
                idx += c_len;
            }
            '>' if brace_depth == 0 => {
                let before = input[attrs_start..idx].trim_end();
                let self_closing = before.ends_with('/');
                return Some((idx + c_len - start, self_closing));
            }
            _ => {
                idx += c_len;
            }
        }
    }

    None
}

fn parse_named_close_tag(input: &str, start: usize, name: &str) -> Option<usize> {
    if !input[start..].starts_with("</") {
        return None;
    }
    let mut idx = start + 2;
    if !input[idx..].starts_with(name) {
        return None;
    }
    idx += name.len();

    let boundary = input[idx..].chars().next()?;
    if !(boundary.is_whitespace() || boundary == '>') {
        return None;
    }

    idx = skip_ws(input, idx);
    if !input[idx..].starts_with('>') {
        return None;
    }
    Some(idx + 1 - start)
}

fn skip_ws(src: &str, mut idx: usize) -> usize {
    while idx < src.len() {
        let Some(c) = src[idx..].chars().next() else {
            break;
        };
        if c.is_whitespace() {
            idx += c.len_utf8();
        } else {
            break;
        }
    }
    idx
}

fn is_rust_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_rust_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

fn consume_char(src: &str, idx: usize, expected: char) -> Option<usize> {
    let c = src[idx..].chars().next()?;
    if c == expected {
        Some(c.len_utf8())
    } else {
        None
    }
}

// ── Island tag transpilation ───────────────────────────────────

/// Transpiles `<island src="..." strategy="..." .../>` tags into
/// silcrow-powered deferred-fetch HTML.
///
/// Each tag compiles to:
/// - A container `<div id="__pi_N" data-pilcrow-island>` wrapper
/// - A hidden `<a s-get="..." s-html s-target="..." s-skip-history>` trigger
/// - A strategy-specific inline `<script>` that clicks the trigger at the right moment
///
/// Transpile all `<pilcrow:image .../>` tags in a template to `<img>` tags that
/// point at the `/_image` optimization endpoint.
///
/// Recognized attributes:
/// - `src`     — required; path to the source image (local file path)
/// - `width`   — output width (px); passed as `?w=`
/// - `height`  — output height (px); passed as `?h=`
/// - `quality` — JPEG/WebP quality 1-100; passed as `?q=`
/// - `format`  — output format (`webp`, `jpeg`, `png`, `auto`); passed as `?f=`
/// - `alt`     — forwarded to `<img alt="...">`
/// - `class`   — forwarded to `<img class="...">`
/// - `sizes`   — forwarded to `<img sizes="...">` (for art direction)
///
/// All other attributes are forwarded verbatim.
///
/// Askama expressions (`{{ ... }}`) inside attribute values are passed through unchanged
/// so dynamic `src`, `width`, etc. work at render time.
///
/// Example:
/// ```html
/// <pilcrow:image src="/public/hero.jpg" width="1200" alt="Hero" />
/// <!-- becomes -->
/// <img src="/_image?src=%2Fpublic%2Fhero.jpg&w=1200" alt="Hero" width="1200" loading="lazy" decoding="async" />
/// ```
pub(crate) fn transpile_pilcrow_tags(template: &str) -> String {
    let mut output = String::with_capacity(template.len());
    let mut i = 0usize;

    while i < template.len() {
        if let Some(consumed) = copy_html_comment(template, i, &mut output) {
            i += consumed;
            continue;
        }

        if template[i..].starts_with("<pilcrow:image") {
            let rest = &template[i + 14..];
            let next = rest.chars().next();
            if matches!(next, Some(c) if c.is_whitespace() || c == '/' || c == '>')
                && let Some((html, consumed)) = parse_pilcrow_image_tag(&template[i..])
            {
                output.push_str(&html);
                i += consumed;
                continue;
            }
        }
        // Strip <pilcrow:head> blocks that survived layout slot expansion
        // (pages with LAYOUT="none" or no _layout.html in the chain).
        if template[i..].starts_with("<pilcrow:head") {
            let rest = &template[i + 13..];
            let next = rest.chars().next();
            if matches!(next, Some(c) if c.is_whitespace() || c == '>')
                && let Some(consumed) = strip_pilcrow_head_block(&template[i..])
            {
                i += consumed;
                continue;
            }
        }
        let c = template[i..].chars().next().unwrap();
        output.push(c);
        i += c.len_utf8();
    }

    output
}

/// Consume a `<pilcrow:head>...</pilcrow:head>` block and return the total bytes consumed.
/// Returns `None` if the block is malformed (no closing tag).
fn strip_pilcrow_head_block(input: &str) -> Option<usize> {
    debug_assert!(input.starts_with("<pilcrow:head"));

    // Scan past the opening tag.
    let mut idx = 13; // len("<pilcrow:head")
    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        idx += c.len_utf8();
        if c == '>' {
            break;
        }
    }

    // Find the matching close tag.
    let close = "</pilcrow:head>";
    input[idx..].find(close).map(|off| idx + off + close.len())
}

fn parse_pilcrow_image_tag(input: &str) -> Option<(String, usize)> {
    debug_assert!(input.starts_with("<pilcrow:image"));

    let mut idx = 14; // skip "<pilcrow:image"
    let mut raw_attrs = String::new();
    let mut quote: Option<char> = None;
    let mut found_end = false;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            raw_attrs.push(c);
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => {
                quote = Some(c);
                raw_attrs.push(c);
                idx += c_len;
            }
            '/' => {
                idx += c_len;
                if input[idx..].starts_with('>') {
                    idx += 1;
                    found_end = true;
                    break;
                }
                raw_attrs.push('/');
            }
            '>' => {
                idx += c_len;
                found_end = true;
                break;
            }
            _ => {
                raw_attrs.push(c);
                idx += c_len;
            }
        }
    }

    if !found_end {
        return None;
    }

    let attrs = parse_attr_map(&raw_attrs);
    let src = attrs.get("src").cloned().unwrap_or_default();

    // Build /_image query string. Dynamic Askama expressions are passed verbatim.
    let mut query_parts: Vec<String> = Vec::new();
    // src is URL-encoded unless it contains `{{` (Askama expression).
    if src.contains("{{") {
        query_parts.push(format!("src={src}"));
    } else {
        query_parts.push(format!("src={}", urlencoding::encode(&src)));
    }
    if let Some(w) = attrs.get("width") {
        query_parts.push(format!("w={w}"));
    }
    if let Some(h) = attrs.get("height") {
        query_parts.push(format!("h={h}"));
    }
    if let Some(q) = attrs.get("quality") {
        query_parts.push(format!("q={q}"));
    }
    if let Some(f) = attrs.get("format") {
        query_parts.push(format!("f={f}"));
    }

    let img_src = format!("/_image?{}", query_parts.join("&"));

    // Build <img> tag, forwarding passthrough attrs.
    let mut img = format!("<img src=\"{img_src}\"");

    for passthrough in &["alt", "class", "id", "style", "sizes"] {
        if let Some(v) = attrs.get(*passthrough) {
            img.push_str(&format!(" {passthrough}=\"{v}\""));
        }
    }
    if let Some(w) = attrs.get("width") {
        img.push_str(&format!(" width=\"{w}\""));
    }
    if let Some(h) = attrs.get("height") {
        img.push_str(&format!(" height=\"{h}\""));
    }
    img.push_str(" loading=\"lazy\" decoding=\"async\"");
    img.push('>');

    Some((img, idx))
}

/// Parse a flat attribute string into a map of name → value pairs.
/// Handles `name="value"`, `name='value'`, and bare `name` attributes.
fn parse_attr_map(raw: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut i = 0usize;
    let bytes = raw.as_bytes();

    while i < raw.len() {
        // skip whitespace
        while i < raw.len()
            && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r')
        {
            i += 1;
        }
        if i >= raw.len() {
            break;
        }

        // read name
        let name_start = i;
        while i < raw.len()
            && bytes[i] != b'='
            && bytes[i] != b' '
            && bytes[i] != b'\t'
            && bytes[i] != b'\n'
        {
            i += 1;
        }
        let name = raw[name_start..i].trim().to_string();
        if name.is_empty() {
            i += 1;
            continue;
        }

        if i >= raw.len() || bytes[i] != b'=' {
            map.insert(name, String::new());
            continue;
        }
        i += 1; // skip '='

        if i >= raw.len() {
            map.insert(name, String::new());
            break;
        }

        let value = if bytes[i] == b'"' || bytes[i] == b'\'' {
            let q = bytes[i] as char;
            i += 1;
            let val_start = i;
            while i < raw.len() && raw.as_bytes()[i] as char != q {
                i += 1;
            }
            let v = raw[val_start..i].to_string();
            if i < raw.len() {
                i += 1;
            } // closing quote
            v
        } else {
            let val_start = i;
            while i < raw.len() && bytes[i] != b' ' && bytes[i] != b'\t' && bytes[i] != b'\n' {
                i += 1;
            }
            raw[val_start..i].to_string()
        };

        map.insert(name, value);
    }

    map
}

/// `page_url_base` is the URL directory of the containing page (e.g. `/dashboard`).
/// Relative `src` values (`./name` or bare `name`) resolve to
/// `{page_url_base}/islands/{name}`. Absolute `src` values (leading `/`) are used as-is.
///
/// All attributes other than `src` and `strategy` are forwarded as URL query parameters.
/// Values may contain Askama expressions — they are passed through verbatim and rendered
/// by Askama when the containing page is rendered.
pub(crate) fn transpile_island_tags(template: &str, page_url_base: &str) -> String {
    let mut output = String::with_capacity(template.len());
    let mut i = 0usize;
    let mut counter = 0usize;

    while i < template.len() {
        if let Some(consumed) = copy_html_comment(template, i, &mut output) {
            i += consumed;
            continue;
        }

        if template[i..].starts_with("<island") {
            let rest = &template[i + 7..];
            let next = rest.chars().next();
            if matches!(next, Some(c) if c.is_whitespace() || c == '/' || c == '>')
                && let Some((html, consumed)) =
                    parse_island_tag(&template[i..], page_url_base, counter)
            {
                output.push_str(&html);
                i += consumed;
                counter += 1;
                continue;
            }
        }
        let c = template[i..].chars().next().unwrap();
        output.push(c);
        i += c.len_utf8();
    }

    output
}

fn copy_html_comment(input: &str, start: usize, output: &mut String) -> Option<usize> {
    if !input[start..].starts_with("<!--") {
        return None;
    }

    let consumed = match input[start + 4..].find("-->") {
        Some(end) => 4 + end + 3,
        None => input.len() - start,
    };
    output.push_str(&input[start..start + consumed]);
    Some(consumed)
}

fn parse_island_tag(input: &str, page_url_base: &str, id: usize) -> Option<(String, usize)> {
    debug_assert!(input.starts_with("<island"));

    let mut idx = 7; // skip "<island"
    let mut raw_attrs = String::new();
    let mut quote: Option<char> = None;
    let mut brace_depth: usize = 0;
    let mut found_end = false;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            raw_attrs.push(c);
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => {
                quote = Some(c);
                raw_attrs.push(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                raw_attrs.push(c);
                idx += c_len;
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                raw_attrs.push(c);
                idx += c_len;
            }
            '/' if brace_depth == 0 => {
                idx += c_len;
                if input[idx..].starts_with('>') {
                    idx += 1;
                    found_end = true;
                    break;
                }
                raw_attrs.push('/');
            }
            '>' if brace_depth == 0 => {
                idx += c_len;
                found_end = true;
                break;
            }
            _ => {
                raw_attrs.push(c);
                idx += c_len;
            }
        }
    }

    if !found_end {
        return None;
    }

    let src = html_attr_value(&raw_attrs, "src")?;
    let strategy = html_attr_value(&raw_attrs, "strategy").unwrap_or_else(|| "load".to_string());

    let url = resolve_island_url(&src, page_url_base);

    // Extra attrs become query params; values are passed verbatim (may be Askama exprs).
    let mut params: Vec<String> = Vec::new();
    for (name, val) in parse_island_attrs(&raw_attrs) {
        if name == "src" || name == "strategy" {
            continue;
        }
        if val.is_empty() {
            params.push(name);
        } else {
            params.push(format!("{name}={val}"));
        }
    }

    let full_url = if params.is_empty() {
        url
    } else {
        format!("{url}?{}", params.join("&"))
    };

    let island_id = format!("__pi_{id}");
    let trigger_id = format!("__pi_{id}t");

    let trigger = format!(
        "<a id=\"{}\" s-get=\"{}\" s-html s-target=\"#{}\" s-skip-history style=\"display:none\"></a>",
        trigger_id, full_url, island_id,
    );

    let script = match strategy.as_str() {
        "visible" => format!(
            "<script>(function(){{var t=document.getElementById('{}');\
             if(!t)return;\
             var o=new IntersectionObserver(function(e){{\
             if(e[0].isIntersecting){{o.disconnect();t.click();}}}});\
             o.observe(document.getElementById('{}'));\
             }})();</script>",
            trigger_id, island_id,
        ),
        "idle" => format!(
            "<script>(function(){{var t=document.getElementById('{}');\
             if(!t)return;\
             'requestIdleCallback'in window?\
             requestIdleCallback(function(){{t.click();}})\
             :setTimeout(function(){{t.click();}},200);\
             }})();</script>",
            trigger_id,
        ),
        _ => format!(
            "<script>document.getElementById('{}').click();</script>",
            trigger_id,
        ),
    };

    let html = format!(
        "<div id=\"{}\" data-pilcrow-island>{}</div>{}",
        island_id, trigger, script,
    );

    Some((html, idx))
}

/// Resolves an island `src` value to an absolute URL.
///
/// - Leading `/` → used as-is (absolute URL)
/// - `./name` or bare `name` → `{page_url_base}/islands/{name}`
fn resolve_island_url(src: &str, page_url_base: &str) -> String {
    if src.starts_with('/') {
        src.to_string()
    } else {
        let name = src.trim_start_matches("./");
        let base = page_url_base.trim_end_matches('/');
        format!("{base}/islands/{name}")
    }
}

/// Parse all name=value pairs from a raw HTML attribute string.
/// Handles `"..."`, `'...'`, and `{...}` value forms.
/// Returns the raw value text (including any `{{ }}` Askama expressions).
fn parse_island_attrs(attrs: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < attrs.len() {
        i = skip_ws(attrs, i);
        if i >= attrs.len() {
            break;
        }
        let (name, next) = scan_html_attr_name(attrs, i);
        if name.is_empty() {
            i += 1;
            continue;
        }
        i = skip_ws(attrs, next);
        if i < attrs.len() && attrs[i..].starts_with('=') {
            i += 1;
            i = skip_ws(attrs, i);
            if let Some((val, end)) = scan_html_attr_val(attrs, i) {
                result.push((name, val));
                i = end;
            } else {
                result.push((name, String::new()));
                i = next;
            }
        } else {
            result.push((name, String::new()));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_html_module_success() {
        let source = r#"---
use crate::models::Post;

pub struct Props {
    pub title: String,
}
---
<h1>{{ title }}</h1>"#;

        let parts = split_html_module(source).expect("expected valid split");
        assert!(parts.rust.contains("pub struct Props"));
        assert_eq!(parts.template, "<h1>{{ title }}</h1>");
    }

    #[test]
    fn split_html_module_allows_missing_fence_as_static_template() {
        let parts = split_html_module("<h1>Only template</h1>").expect("expected valid split");
        assert_eq!(parts.rust, "");
        assert_eq!(parts.template, "<h1>Only template</h1>");
    }

    #[test]
    fn split_html_module_rejects_empty_template() {
        let err = split_html_module("---\nlet x = 1;\n---\n\n").expect_err("expected an error");
        assert_eq!(err, HtmlModuleParseError::EmptyTemplate);
    }

    #[test]
    fn transpile_component_tag_basic() {
        let input = r#"<Card title={item.title} active={item.active} />"#;
        let output = transpile_component_tags(input);
        assert_eq!(
            output,
            "{{ Card { title: item.title, active: item.active }|safe }}"
        );
    }

    #[test]
    fn transpile_component_tag_askama_quoted_expr() {
        let input = r#"<Card title="{{ item.title }}" />"#;
        let output = transpile_component_tags(input);
        assert_eq!(output, "{{ Card { title: item.title }|safe }}");
    }

    #[test]
    fn transpile_component_tag_string_literal() {
        let input = r#"<Badge label="new" />"#;
        let output = transpile_component_tags(input);
        assert_eq!(output, "{{ Badge { label: \"new\" }|safe }}");
    }

    #[test]
    fn transpile_component_tag_without_props() {
        let input = "<Footer />";
        let output = transpile_component_tags(input);
        assert_eq!(output, "{{ Footer {}|safe }}");
    }

    #[test]
    fn transpile_component_with_paired_children() {
        let input = "<Layout title={title}><h1>Hello</h1></Layout>";
        let output = transpile_component_tags(input);
        assert_eq!(
            output,
            "{{ Layout { title: title, children: r\"<h1>Hello</h1>\" }|safe }}"
        );
    }

    #[test]
    fn transpile_component_with_nested_children_components() {
        let input = "<Layout title={title}><Card title={title} /></Layout>";
        let output = transpile_component_tags(input);
        assert_eq!(
            output,
            "{{ Layout { title: title, children: r\"{{ Card { title: title }|safe }}\" }|safe }}"
        );
    }

    #[test]
    fn transpile_component_with_empty_paired_body() {
        let input = "<Footer></Footer>";
        let output = transpile_component_tags(input);
        assert_eq!(output, "{{ Footer {}|safe }}");
    }

    #[test]
    fn transpile_component_handles_nested_same_name_tags() {
        let input = "<Box><Box /></Box>";
        let output = transpile_component_tags(input);
        assert_eq!(
            output,
            "{{ Box { children: r\"{{ Box {}|safe }}\" }|safe }}"
        );
    }

    #[test]
    fn transpile_component_ignores_tags_inside_html_comments() {
        let input = r#"<!-- <Card title={hidden} /> --><Card title={visible} />"#;
        let output = transpile_component_tags(input);

        assert!(output.contains("<!-- <Card title={hidden} /> -->"));
        assert!(output.contains("{{ Card { title: visible }|safe }}"));
    }

    #[test]
    fn transpile_ignores_lowercase_html_tags() {
        let input = r#"<div class="x"><span>Hi</span></div>"#;
        let output = transpile_component_tags(input);
        assert_eq!(output, input);
    }

    #[test]
    fn transpile_ignores_invalid_component_attrs() {
        let input = r#"<Card s-key=".id" title=".title" />"#;
        let output = transpile_component_tags(input);
        assert_eq!(output, input);
    }

    // ── inject_form_method_attrs ──────────────────────────────

    #[test]
    fn inject_form_injects_method_and_action_for_s_post() {
        let input = r##"<form s-post="?action=create" s-target="#f">"##;
        let output = inject_form_method_attrs(input);
        assert!(
            output.contains(r#"method="post""#),
            "should inject method: {output}"
        );
        assert!(
            output.contains(r#"action="?action=create""#),
            "should inject action: {output}"
        );
    }

    #[test]
    fn inject_form_does_not_overwrite_existing_method() {
        let input = r#"<form s-post="?action=create" method="POST">"#;
        let output = inject_form_method_attrs(input);
        // Only one method= present
        assert_eq!(output.matches("method=").count(), 1);
    }

    #[test]
    fn inject_form_does_not_overwrite_existing_action() {
        let input = r#"<form s-post="?action=create" action="/custom">"#;
        let output = inject_form_method_attrs(input);
        assert!(output.contains(r#"action="/custom""#));
        assert!(!output.contains(r#"action="?action=create""#));
    }

    #[test]
    fn inject_form_ignores_s_get() {
        let input = r#"<form s-get="/search">"#;
        let output = inject_form_method_attrs(input);
        assert_eq!(input, output, "s-get should not be modified");
    }

    #[test]
    fn inject_form_handles_s_delete() {
        let input = r#"<form s-delete="/items/1">"#;
        let output = inject_form_method_attrs(input);
        assert!(output.contains(r#"method="post""#));
        assert!(output.contains(r#"action="/items/1""#));
    }

    #[test]
    fn inject_form_ignores_non_form_elements() {
        let input = r#"<div s-post="?action=create"></div>"#;
        let output = inject_form_method_attrs(input);
        assert_eq!(input, output, "non-form elements should not be modified");
    }

    #[test]
    fn inject_form_leaves_forms_without_verb_attrs_untouched() {
        let input = r#"<form id="search" class="form">"#;
        let output = inject_form_method_attrs(input);
        assert_eq!(input, output);
    }

    #[test]
    fn inject_form_handles_multiple_forms() {
        let input = r#"<form s-post="?action=login"><form s-post="?action=signup">"#;
        let output = inject_form_method_attrs(input);
        assert_eq!(output.matches(r#"method="post""#).count(), 2);
        assert!(output.contains(r#"action="?action=login""#));
        assert!(output.contains(r#"action="?action=signup""#));
    }

    #[test]
    fn transpile_ignores_mismatched_paired_tags() {
        let input = "<Layout><Card /></Layot>";
        let output = transpile_component_tags(input);
        assert_eq!(output, "<Layout>{{ Card {}|safe }}</Layot>");
    }

    #[test]
    fn transpile_handles_multiple_components() {
        let input = r#"
{% for item in items %}
    <Card title={item.title} />
    <Badge label="new" />
{% endfor %}
"#;

        let output = transpile_component_tags(input);
        assert!(output.contains("{{ Card { title: item.title }|safe }}"));
        assert!(output.contains("{{ Badge { label: \"new\" }|safe }}"));
    }

    #[test]
    fn transpile_html_module_runs_split_and_template_transform() {
        let source = r#"---
pub struct Props { pub title: String }
---
<Card title={title} />"#;

        let parts = transpile_html_module(source).expect("should parse/transpile");
        assert!(parts.rust.contains("pub struct Props"));
        assert_eq!(parts.template, "{{ Card { title: title }|safe }}");
    }

    #[test]
    fn island_load_strategy_emits_click_script() {
        let input = r#"<island src="./counter" />"#;
        let out = transpile_island_tags(input, "/dashboard");
        assert!(out.contains("data-pilcrow-island"));
        assert!(out.contains(r#"s-get="/dashboard/islands/counter""#));
        assert!(out.contains("s-html"));
        assert!(out.contains(".click()"));
        assert!(!out.contains("IntersectionObserver"));
    }

    #[test]
    fn island_visible_strategy_emits_intersection_observer() {
        let input = r#"<island src="./user-card" strategy="visible" />"#;
        let out = transpile_island_tags(input, "/profile");
        assert!(out.contains("IntersectionObserver"));
        assert!(out.contains(r#"s-get="/profile/islands/user-card""#));
        assert!(!out.contains("requestIdleCallback"));
    }

    #[test]
    fn island_idle_strategy_emits_idle_callback() {
        let input = r#"<island src="./feed" strategy="idle" />"#;
        let out = transpile_island_tags(input, "/home");
        assert!(out.contains("requestIdleCallback"));
        assert!(out.contains(r#"s-get="/home/islands/feed""#));
        assert!(!out.contains("IntersectionObserver"));
    }

    #[test]
    fn island_absolute_src_is_used_as_is() {
        let input = r#"<island src="/shared/user-card" />"#;
        let out = transpile_island_tags(input, "/dashboard");
        assert!(out.contains(r#"s-get="/shared/user-card""#));
    }

    #[test]
    fn island_extra_attrs_become_query_params() {
        let input = r#"<island src="./counter" count="5" label="hello" />"#;
        let out = transpile_island_tags(input, "/dashboard");
        assert!(out.contains("count=5"));
        assert!(out.contains("label=hello"));
    }

    #[test]
    fn island_sequential_ids_are_unique() {
        let input = r#"<island src="./a" /><island src="./b" />"#;
        let out = transpile_island_tags(input, "/page");
        assert!(out.contains("__pi_0"));
        assert!(out.contains("__pi_1"));
    }

    #[test]
    fn island_does_not_match_non_island_tags() {
        let input = "<islander>content</islander>";
        let out = transpile_island_tags(input, "/page");
        assert_eq!(out, input);
    }

    #[test]
    fn island_ignores_tags_inside_html_comments() {
        let input = r#"<!-- <island src="./hidden" /> --><island src="./visible" />"#;
        let out = transpile_island_tags(input, "/page");

        assert!(out.contains(r#"<!-- <island src="./hidden" /> -->"#));
        assert!(!out.contains(r#"s-get="/page/islands/hidden""#));
        assert!(out.contains(r#"s-get="/page/islands/visible""#));
    }

    #[test]
    fn resolve_island_url_relative_without_dot_slash() {
        assert_eq!(
            resolve_island_url("counter", "/dashboard"),
            "/dashboard/islands/counter"
        );
    }

    #[test]
    fn resolve_island_url_relative_with_dot_slash() {
        assert_eq!(
            resolve_island_url("./counter", "/dashboard"),
            "/dashboard/islands/counter"
        );
    }

    #[test]
    fn resolve_island_url_absolute() {
        assert_eq!(
            resolve_island_url("/shared/widget", "/dashboard"),
            "/shared/widget"
        );
    }

    // ── <pilcrow:image> transpiler tests ─────────────────────────────────────

    #[test]
    fn pilcrow_image_basic_src_and_width() {
        let input = r#"<pilcrow:image src="/public/hero.jpg" width="1200" alt="Hero" />"#;
        let out = transpile_pilcrow_tags(input);
        assert!(out.contains("/_image?"), "should point to /_image");
        assert!(out.contains("w=1200"), "should pass width");
        assert!(out.contains(r#"alt="Hero""#), "should forward alt");
        assert!(out.contains(r#"width="1200""#), "should set width attr");
        assert!(out.contains(r#"loading="lazy""#), "should be lazy");
        assert!(out.contains(r#"decoding="async""#), "should be async");
    }

    #[test]
    fn pilcrow_image_without_width_has_no_w_param() {
        let input = r#"<pilcrow:image src="/images/photo.png" alt="Photo" />"#;
        let out = transpile_pilcrow_tags(input);
        assert!(out.contains("/_image?src="), "should have src param");
        assert!(
            !out.contains("w="),
            "should not have w param when width absent"
        );
    }

    #[test]
    fn pilcrow_image_format_and_quality() {
        let input =
            r#"<pilcrow:image src="/img/banner.jpg" format="webp" quality="90" width="800" />"#;
        let out = transpile_pilcrow_tags(input);
        assert!(out.contains("f=webp"), "should pass format");
        assert!(out.contains("q=90"), "should pass quality");
    }

    #[test]
    fn pilcrow_image_src_is_url_encoded() {
        let input = r#"<pilcrow:image src="/public/my image.jpg" alt="test" />"#;
        let out = transpile_pilcrow_tags(input);
        assert!(
            out.contains("%20") || out.contains("+"),
            "space should be encoded"
        );
    }

    #[test]
    fn pilcrow_image_class_forwarded() {
        let input = r#"<pilcrow:image src="/img.jpg" class="hero-img" alt="img" />"#;
        let out = transpile_pilcrow_tags(input);
        assert!(
            out.contains(r#"class="hero-img""#),
            "class should be forwarded"
        );
    }

    #[test]
    fn pilcrow_image_does_not_match_non_image_tags() {
        let input = r#"<pilcrow:other src="/img.jpg" />"#;
        let out = transpile_pilcrow_tags(input);
        assert_eq!(input, out, "non-image pilcrow tags should pass through");
    }

    #[test]
    fn pilcrow_image_mixed_content_unchanged_parts() {
        let input = r#"<div><pilcrow:image src="/img.jpg" alt="x" /><p>Hello</p></div>"#;
        let out = transpile_pilcrow_tags(input);
        assert!(out.contains("<div>"), "surrounding HTML preserved");
        assert!(out.contains("<p>Hello</p>"), "surrounding HTML preserved");
        assert!(out.contains("/_image?"), "image tag transpiled");
    }

    // ── <pilcrow:head> orphan stripping ──────────────────────────────────────

    #[test]
    fn pilcrow_head_stripped_when_no_layout() {
        let input = "<pilcrow:head><title>My Page</title></pilcrow:head><h1>Body</h1>";
        let out = transpile_pilcrow_tags(input);
        assert!(
            !out.contains("<pilcrow:head>"),
            "pilcrow:head should be removed"
        );
        assert!(!out.contains("<title>"), "head content should be stripped");
        assert!(out.contains("<h1>Body</h1>"), "body content preserved");
    }

    #[test]
    fn pilcrow_head_stripped_with_multiple_meta_tags() {
        let input = concat!(
            "<pilcrow:head>",
            "<title>Shop</title>",
            r#"<meta name="description" content="Browse products" />"#,
            r#"<meta property="og:title" content="Shop" />"#,
            "</pilcrow:head>",
            "<p>Content</p>",
        );
        let out = transpile_pilcrow_tags(input);
        assert!(!out.contains("<pilcrow:head>"), "pilcrow:head tag removed");
        assert!(!out.contains("<title>"), "title stripped");
        assert!(!out.contains("<meta"), "meta tags stripped");
        assert!(out.contains("<p>Content</p>"), "body content preserved");
    }

    #[test]
    fn pilcrow_head_with_whitespace_after_tag_name_stripped() {
        let input = "<pilcrow:head>\n  <title>T</title>\n</pilcrow:head>\n<p>X</p>";
        let out = transpile_pilcrow_tags(input);
        assert!(!out.contains("pilcrow:head"), "tag removed");
        assert!(out.contains("<p>X</p>"), "body preserved");
    }

    #[test]
    fn pilcrow_head_does_not_match_pilcrow_image() {
        let input = r#"<pilcrow:image src="/img.jpg" alt="x" />"#;
        let out = transpile_pilcrow_tags(input);
        assert!(out.starts_with("<img"), "image tag still processed");
    }
}
