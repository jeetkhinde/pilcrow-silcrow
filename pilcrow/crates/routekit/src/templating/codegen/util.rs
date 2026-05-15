use super::*;

/// Return the last path segment ident for a type like `foo::Bar<T>` → `Bar`.
pub fn type_last_ident(ty: &syn::Type) -> Option<&syn::Ident> {
    let syn::Type::Path(tp) = ty else { return None };
    tp.path.segments.last().map(|seg| &seg.ident)
}

/// Return the identifier string for each named field.
pub fn named_field_names(fields: &[syn::Field]) -> Vec<String> {
    fields
        .iter()
        .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
        .collect()
}

pub fn build_symbol_name(template_path: &str, pages_dir_norm: &str) -> String {
    let path = normalize_path_text(Path::new(template_path));
    let relative = path
        .strip_prefix(pages_dir_norm)
        .unwrap_or(&path)
        .trim_start_matches('/');
    let without_ext = relative.strip_suffix(".html").unwrap_or(relative);

    // Strip layout-group segments `(group)` — they don't appear in URLs or module names.
    let stripped: String = without_ext
        .split('/')
        .filter(|seg| !(seg.starts_with('(') && seg.ends_with(')')))
        .collect::<Vec<_>>()
        .join("/");
    let without_ext = stripped.as_str();

    let mut symbol = String::new();
    let mut prev_underscore = false;
    for ch in without_ext.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };

        if mapped == '_' {
            if !prev_underscore {
                symbol.push('_');
            }
            prev_underscore = true;
        } else {
            symbol.push(mapped);
            prev_underscore = false;
        }
    }

    let symbol = symbol.trim_matches('_');
    let symbol = if symbol.is_empty() { "index" } else { symbol };
    format!("page_{symbol}")
}

pub fn normalize_path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

/// Sanitize an arbitrary string into a valid Rust module-name segment (lowercase alphanumeric + `_`).
pub fn sanitize_module_segment(s: &str) -> String {
    let mut out = String::new();
    let mut prev_underscore = false;
    for ch in s.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if mapped == '_' {
            if !prev_underscore {
                out.push('_');
            }
            prev_underscore = true;
        } else {
            out.push(mapped);
            prev_underscore = false;
        }
    }
    out.trim_matches('_').to_string()
}

/// Extract the leaf name for a fragment module by stripping the `frag_{sanitized_prefix}_` prefix.
///
/// `frag_widgets_product_list` with prefix `widgets` → `product_list`
pub fn fragment_leaf_name(module_name: &str, url_prefix: &str) -> String {
    let sanitized = sanitize_module_segment(url_prefix);
    let strip = format!("frag_{sanitized}_");
    module_name
        .strip_prefix(&strip)
        .unwrap_or(module_name)
        .to_string()
}
