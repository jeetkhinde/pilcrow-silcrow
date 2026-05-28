use quote::ToTokens;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io;
use std::path::Path;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, Ident, LitInt, Meta, Token};

use crate::templating::page_options::LiveFieldAttr;

/// Field extracted from the `Live` struct in `live.rs`.
pub struct LiveField {
    pub name: String,
    pub column_name: Option<String>,
    /// The inner type T in LiveProp<T>.
    pub inner_type: String,
    pub depends_on: Option<DependencyExpr>,
    pub patch_debounce: Option<u32>,
    pub allow_unused: bool,
}

#[derive(Clone)]
pub enum DependencyExpr {
    DepMacro {
        table: String,
        column: String,
        value: RuntimeValueExpr,
    },
    Expr(String),
}

#[derive(Clone)]
pub enum RuntimeValueExpr {
    ParamsField(String),
    Expr(String),
}

#[derive(Default)]
struct LiveDefaults {
    column_name: Option<String>,
    depends_on: Option<DependencyExpr>,
    patch_debounce: Option<u32>,
    allow_unused: bool,
}

struct LiveSlotUse {
    name: String,
    count: usize,
}

struct DepMacroInput {
    table: Ident,
    column: Ident,
    value: Expr,
}

impl Parse for DepMacroInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let table = input.parse::<Ident>()?;
        input.parse::<Token![,]>()?;
        let column = input.parse::<Ident>()?;
        input.parse::<Token![,]>()?;
        let value = input.parse::<Expr>()?;
        Ok(Self {
            table,
            column,
            value,
        })
    }
}

struct DependsOnRouteInput {
    table: Ident,
    column: Ident,
}

impl Parse for DependsOnRouteInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let table = input.parse::<Ident>()?;
        input.parse::<Token![,]>()?;
        let column = input.parse::<Ident>()?;
        Ok(Self { table, column })
    }
}

/// Process a `live.rs` file: strip `#[pilcrow::*]` attrs, extract LiveProp fields,
/// and generate a `from_row()` impl.
///
/// `route_promote_after` — from the page-level `PROMOTE_AFTER` constant. When `Some`,
/// emits `route_promote_after()` so the runtime uses this threshold to promote the route.
///
/// `auto_attrs` — per-field options from `#[pilcrow::live(...)]` on Props `LiveProp<T>` fields.
/// For each field: if no explicit `depends_on` is set in `live.rs` and `revalidate_secs` is set,
/// auto-injects dep key `"{module_name}::{field_name}"` as `depends_on`.
///
/// `module_name` — the page module symbol (e.g. `"page_tickets"`), used to derive dep keys.
///
/// Returns (processed_source, live_fields).
pub fn process_live_rs(
    path: &Path,
    route_params: &[String],
    route_promote_after: Option<u32>,
    auto_attrs: &HashMap<String, LiveFieldAttr>,
    module_name: &str,
) -> io::Result<(String, Vec<LiveField>)> {
    let source = std::fs::read_to_string(path)?;
    let mut file: syn::File = syn::parse_str(&source).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse live.rs: {e}"),
        )
    })?;

    // Find the Live struct and extract + clean its fields.
    let mut live_fields: Vec<LiveField> = Vec::new();
    let route_params: HashSet<&str> = route_params.iter().map(String::as_str).collect();

    for item in &mut file.items {
        let syn::Item::Struct(s) = item else { continue };
        if s.ident != "Live" {
            continue;
        }

        let struct_defaults = parse_live_defaults(&s.attrs, &route_params)?;
        s.attrs.retain(|attr| !is_pilcrow_attr(attr));

        // Ensure Live derives serde::Serialize so it can be a field in Props (which
        // gets #[derive(Serialize)] injected by codegen).
        let has_serialize = s.attrs.iter().any(|attr| {
            let path = attr.path();
            let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();
            (segments == ["derive"] || segments == ["serde", "Serialize"])
                && attr.to_token_stream().to_string().contains("Serialize")
        });
        if !has_serialize {
            s.attrs.push(syn::parse_quote!(#[derive(::serde::Serialize)]));
        }

        let syn::Fields::Named(named) = &mut s.fields else {
            continue;
        };
        for field in &mut named.named {
            let field_defaults = parse_field_defaults(&field.attrs, &route_params)?;

            // Strip all #[pilcrow::*] attributes.
            field.attrs.retain(|attr| !is_pilcrow_attr(attr));

            // Collect LiveProp<T> fields.
            let Some(ident) = &field.ident else { continue };
            let is_live_props = if let syn::Type::Path(tp) = &field.ty {
                tp.path
                    .segments
                    .last()
                    .is_some_and(|seg| seg.ident == "LiveProp")
            } else {
                false
            };

            if is_live_props {
                // Extract inner type T from LiveProp<T>.
                let inner = extract_live_props_inner(&field.ty)
                    .unwrap_or_else(|| "serde_json::Value".to_string());
                live_fields.push(LiveField {
                    name: ident.to_string(),
                    column_name: field_defaults.column_name,
                    inner_type: inner,
                    depends_on: field_defaults
                        .depends_on
                        .or_else(|| struct_defaults.depends_on.clone()),
                    patch_debounce: field_defaults
                        .patch_debounce
                        .or(struct_defaults.patch_debounce),
                    allow_unused: field_defaults.allow_unused,
                });
            }
        }
        break;
    }

    // Generate from_row() impl and append to file.
    if !live_fields.is_empty() {
        let from_row_impl = generate_from_row_impl(&live_fields, route_promote_after, auto_attrs, module_name);
        let mut out = file.into_token_stream().to_string();
        out.push('\n');
        out.push_str(&from_row_impl);
        return Ok((out, live_fields));
    }

    Ok((file.into_token_stream().to_string(), live_fields))
}

/// Validate a page template's `s-live` slots against its sibling `live.rs` fields.
pub fn validate_live_template_slots(
    template_source: &str,
    live_fields: &[LiveField],
    template_path: &Path,
    live_path: &Path,
) -> io::Result<()> {
    let slots = collect_s_live_slots(template_source);
    let mut errors = Vec::new();

    for slot in slots.values().filter(|slot| slot.count > 1) {
        errors.push(format!(
            "duplicate s-live slot `{}` appears {} times in {}; each FSR slot must be unique so initial HTML baking and live patches target one element clearly",
            slot.name,
            slot.count,
            template_path.display()
        ));
    }

    let field_names: HashSet<&str> = live_fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    let slot_names: HashSet<&str> = slots.keys().map(String::as_str).collect();

    for slot_name in &slot_names {
        if !field_names.contains(slot_name) {
            errors.push(format!(
                "s-live=\"{}\" exists in {} but no matching Live field exists in {}",
                slot_name,
                template_path.display(),
                live_path.display()
            ));
        }
    }

    for field in live_fields {
        if !field.allow_unused && !slot_names.contains(field.name.as_str()) {
            errors.push(format!(
                "Live field `{}` exists in {} but no matching s-live=\"{}\" exists in {}; add the slot or mark the field with #[pilcrow::allow_unused]",
                field.name,
                live_path.display(),
                field.name,
                template_path.display()
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid FSR live slot configuration between {} and {}:
- {}",
                template_path.display(),
                live_path.display(),
                errors.join("
- ")
            ),
        ))
    }
}

fn collect_s_live_slots(html: &str) -> BTreeMap<String, LiveSlotUse> {
    let mut slots = BTreeMap::new();
    let mut offset = 0;
    while let Some(pos) = html[offset..].find("s-live") {
        let attr_start = offset + pos;
        let after_name = attr_start + "s-live".len();
        if after_name < html.len()
            && html[after_name..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            offset = after_name;
            continue;
        }

        let Some((name, next_offset)) = parse_s_live_value(html, after_name) else {
            offset = after_name;
            continue;
        };
        if !name.is_empty() {
            slots
                .entry(name.clone())
                .and_modify(|slot: &mut LiveSlotUse| slot.count += 1)
                .or_insert(LiveSlotUse { name, count: 1 });
        }
        offset = next_offset;
    }
    slots
}

fn parse_s_live_value(html: &str, mut offset: usize) -> Option<(String, usize)> {
    let bytes = html.as_bytes();
    while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
        offset += 1;
    }
    if bytes.get(offset) != Some(&b'=') {
        return None;
    }
    offset += 1;
    while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
        offset += 1;
    }
    let quote = *bytes.get(offset)?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    offset += 1;
    let value_start = offset;
    while offset < bytes.len() && bytes[offset] != quote {
        offset += 1;
    }
    if offset >= bytes.len() {
        return None;
    }
    Some((html[value_start..offset].to_string(), offset + 1))
}

fn extract_live_props_inner(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(tp) = ty {
        let seg = tp.path.segments.last()?;
        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
            let first = args.args.first()?;
            return Some(first.to_token_stream().to_string());
        }
    }
    None
}

fn is_pilcrow_attr(attr: &syn::Attribute) -> bool {
    attr.path()
        .segments
        .first()
        .is_some_and(|seg| seg.ident == "pilcrow")
}

fn parse_live_defaults(
    attrs: &[syn::Attribute],
    route_params: &HashSet<&str>,
) -> io::Result<LiveDefaults> {
    let mut defaults = LiveDefaults::default();
    for attr in attrs {
        if !is_pilcrow_attr(attr) {
            continue;
        }
        let Some(last) = attr.path().segments.last() else {
            continue;
        };
        if last.ident == "live" {
            if let Ok(items) = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            {
                for item in items {
                    let Meta::NameValue(name_value) = item else {
                        continue;
                    };
                    if name_value.path.is_ident("patch_debounce") {
                        defaults.patch_debounce = expr_to_u32(&name_value.value);
                    } else if name_value.path.is_ident("depends_on") {
                        defaults.depends_on = Some(parse_dependency_expr(&name_value.value));
                    }
                }
            }
        } else if last.ident == "depends_on_route" {
            defaults.depends_on = Some(parse_depends_on_route_attr(attr, route_params)?);
        } else if last.ident == "allow_unused" {
            defaults.allow_unused = true;
        }
    }
    Ok(defaults)
}

fn parse_field_defaults(
    attrs: &[syn::Attribute],
    route_params: &HashSet<&str>,
) -> io::Result<LiveDefaults> {
    let mut defaults = LiveDefaults::default();
    for attr in attrs {
        if !is_pilcrow_attr(attr) {
            continue;
        }
        let Some(last) = attr.path().segments.last() else {
            continue;
        };
        if last.ident == "patch_debounce" {
            defaults.patch_debounce = attr
                .parse_args::<LitInt>()
                .ok()
                .and_then(|lit| lit.base10_parse::<u32>().ok());
        } else if last.ident == "depends_on"
            && let Ok(expr) = attr.parse_args::<Expr>()
        {
            defaults.depends_on = Some(parse_dependency_expr(&expr));
        } else if last.ident == "depends_on_route" {
            defaults.depends_on = Some(parse_depends_on_route_attr(attr, route_params)?);
        } else if last.ident == "allow_unused" {
            defaults.allow_unused = true;
        } else if last.ident == "column" {
            if let Ok(lit) = attr.parse_args::<syn::LitStr>() {
                defaults.column_name = Some(lit.value());
            }
        }
    }
    Ok(defaults)
}

fn parse_depends_on_route_attr(
    attr: &syn::Attribute,
    route_params: &HashSet<&str>,
) -> io::Result<DependencyExpr> {
    let input = attr.parse_args::<DependsOnRouteInput>().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid #[pilcrow::depends_on_route(table, column)] attribute in live.rs: {e}"
            ),
        )
    })?;
    let param = input.column.to_string();
    if !route_params.contains(param.as_str()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid #[pilcrow::depends_on_route({}, {})] in live.rs: route param `{}` is missing. Use a route like [id] or use #[pilcrow::depends_on(dep!(...))] for an explicit runtime value.",
                input.table, input.column, param
            ),
        ));
    }
    Ok(DependencyExpr::DepMacro {
        table: input.table.to_string(),
        column: input.column.to_string(),
        value: RuntimeValueExpr::ParamsField(param),
    })
}

fn expr_to_u32(expr: &Expr) -> Option<u32> {
    match expr {
        Expr::Lit(lit) => {
            if let syn::Lit::Int(int) = &lit.lit {
                int.base10_parse::<u32>().ok()
            } else {
                None
            }
        }
        _ => None,
    }
}

fn parse_dependency_expr(expr: &Expr) -> DependencyExpr {
    if let Expr::Macro(expr_macro) = expr
        && expr_macro
            .mac
            .path
            .segments
            .last()
            .is_some_and(|seg| seg.ident == "dep")
        && let Ok(input) = syn::parse2::<DepMacroInput>(expr_macro.mac.tokens.clone())
    {
        return DependencyExpr::DepMacro {
            table: input.table.to_string(),
            column: input.column.to_string(),
            value: parse_runtime_value_expr(&input.value),
        };
    }
    DependencyExpr::Expr(expr.to_token_stream().to_string())
}

fn parse_runtime_value_expr(expr: &Expr) -> RuntimeValueExpr {
    if let Expr::Field(field) = expr
        && let Expr::Path(base) = field.base.as_ref()
        && base.path.is_ident("params")
        && let syn::Member::Named(member) = &field.member
    {
        return RuntimeValueExpr::ParamsField(member.to_string());
    }
    RuntimeValueExpr::Expr(expr.to_token_stream().to_string())
}

fn generate_from_row_impl(
    fields: &[LiveField],
    route_promote_after: Option<u32>,
    auto_attrs: &HashMap<String, LiveFieldAttr>,
    module_name: &str,
) -> String {
    let mut out = String::from("impl ::pilcrow_runtime::fsr::PilcrowLive for Live {
");
    out.push_str("    fn query(_params: &::serde_json::Map<String, ::serde_json::Value>) -> ::pilcrow_runtime::fsr::LiveQuery {
");
    out.push_str("        Live::query(_params)\n");
    out.push_str("    }
");
    out.push_str("    fn from_row(row: &::std::collections::HashMap<String, ::serde_json::Value>, _params: &::serde_json::Map<String, ::serde_json::Value>) -> Self {
");
    out.push_str("        Self {
");
    for field in fields {
        let name = &field.name;
        let column_name = field.column_name.as_ref().unwrap_or(name);
        // Resolve depends_on priority: explicit live.rs > #[depends_on("key")] > #[revalidate(N)] deduped timer > default timer
        let effective_depends_on = if field.depends_on.is_some() {
            // Explicit live.rs dep wins; this field is DB-driven, no revalidation timer.
            generate_depends_on(&field.depends_on)
        } else {
            let dep_key = auto_attrs.get(name)
                .and_then(|a| {
                    if let Some(ref key) = a.depends_on {
                        Some(key.clone())  // #[depends_on("key")] — static dep, no timer
                    } else if let Some(secs) = a.revalidate_secs {
                        Some(format!("{module_name}::__revalidate_{secs}s"))  // shared per-interval timer
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| format!("{module_name}::__revalidate_default"));
            // Use {:?} to safely escape the dep key as a Rust string literal.
            format!("::std::vec![{dep_key:?}.to_string()]")
        };
        out.push_str(&format!(
            "            {name}: ::pilcrow_runtime::fsr::LiveProp {{
"
        ));
        out.push_str(&format!("                value: row.get(\"{column_name}\")\n"));
        out.push_str(
            "                    .and_then(|v| ::serde_json::from_value(v.clone()).ok())
",
        );
        out.push_str("                    .unwrap_or_default(),
");
        out.push_str("                depends_on: ");
        out.push_str(&effective_depends_on);
        out.push_str(",
");
        out.push_str("                patch_debounce: ");
        out.push_str(&generate_option_u32(field.patch_debounce));
        out.push_str(",
");
        out.push_str("            },
");
    }
    out.push_str("        }
    }
");
    // Generate live_fields() impl.
    out.push_str("    fn live_fields(_params: &::serde_json::Map<String, ::serde_json::Value>) -> ::std::vec::Vec<::pilcrow_runtime::fsr::LiveFieldRegistration> {\n");
    out.push_str("        ::std::vec![\n");
    for field in fields {
        let name = &field.name;
        let effective_depends_on_vec = if field.depends_on.is_some() {
            generate_depends_on_vec(&field.depends_on)
        } else {
            let dep_key = auto_attrs.get(name)
                .and_then(|a| {
                    if let Some(ref key) = a.depends_on {
                        Some(key.clone())
                    } else if let Some(secs) = a.revalidate_secs {
                        Some(format!("{module_name}::__revalidate_{secs}s"))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| format!("{module_name}::__revalidate_default"));
            format!("::std::vec![{dep_key:?}.to_string()]")
        };

        out.push_str("            ::pilcrow_runtime::fsr::LiveFieldRegistration {\n");
        out.push_str(&format!("                slot: \"{}\",\n", field.name));
        match &field.column_name {
            Some(col) => out.push_str(&format!(
                "                column_name: ::std::option::Option::Some(\"{col}\"),\n"
            )),
            None => out.push_str("                column_name: ::std::option::Option::None,\n"),
        }
        out.push_str("                depends_on: ");
        out.push_str(&effective_depends_on_vec);
        out.push_str(",\n");
        out.push_str("                patch_debounce: ");
        out.push_str(&generate_option_u32(field.patch_debounce));
        out.push_str(",\n");
        out.push_str("            },\n");
    }
    out.push_str("        ]\n");
    out.push_str("    }\n");
    // Emit route_promote_after() only when a page-level PROMOTE_AFTER / PRERENDER = true
    // was set. The default trait impl returns None, which preserves existing behaviour.
    if let Some(n) = route_promote_after {
        out.push_str(&format!(
            "    fn route_promote_after() -> ::std::option::Option<u32> {{ ::std::option::Option::Some({n}) }}\n"
        ));
    }
    out.push_str("}\n");
    out
}

fn generate_option_u32(value: Option<u32>) -> String {
    match value {
        Some(value) => format!("::std::option::Option::Some({value})"),
        None => "::std::option::Option::None".to_string(),
    }
}

fn generate_depends_on(depends_on: &Option<DependencyExpr>) -> String {
    match depends_on {
        Some(dep) => format!("::std::vec![{}]", generate_dependency_string(dep)),
        None => "::std::vec![]".to_string(),
    }
}

fn generate_dependency_string(dep: &DependencyExpr) -> String {
    match dep {
        DependencyExpr::DepMacro {
            table,
            column,
            value,
        } => format!(
            "::std::format!(\"{}:{}={{}}\", {})",
            table,
            column,
            generate_runtime_value(value)
        ),
        DependencyExpr::Expr(expr) => format!("({expr}).as_dep_string()"),
    }
}

/// Generate a `Vec<String>` literal for the `depends_on` field of `LiveFieldRegistration`.
/// This is evaluated at runtime (not compile time), unlike `generate_depends_on` which
/// is used for `LiveProp.depends_on` (also runtime, but in a different context).
fn generate_depends_on_vec(depends_on: &Option<DependencyExpr>) -> String {
    match depends_on {
        None => "::std::vec![]".to_string(),
        Some(dep) => format!("::std::vec![{}]", generate_dependency_string(dep)),
    }
}

fn generate_runtime_value(value: &RuntimeValueExpr) -> String {
    match value {
        RuntimeValueExpr::ParamsField(field) => format!(
            "_params.get(\"{field}\").map(|v| match v {{ ::serde_json::Value::String(s) => s.clone(), _ => v.to_string() }}).unwrap_or_default()"
        ),
        RuntimeValueExpr::Expr(expr) => expr.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_live_rs(source: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "pilcrow-routekit-live-{}-{}.rs",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, source).expect("write temp live.rs");
        path
    }

    fn live_field(name: &str) -> LiveField {
        LiveField {
            name: name.to_string(),
            column_name: None,
            inner_type: "String".to_string(),
            depends_on: None,
            patch_debounce: None,
            allow_unused: false,
        }
    }

    fn assert_live_validation_snapshot(template: &str, fields: &[LiveField], expected: &str) {
        let err = validate_live_template_slots(
            template,
            fields,
            Path::new("pages/tickets/page.html"),
            Path::new("pages/tickets/live.rs"),
        )
        .expect_err("validation should fail");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert_eq!(err.to_string(), expected);
    }

    #[test]
    fn live_slot_validation_errors_when_template_slot_has_no_live_field() {
        assert_live_validation_snapshot(
            r#"<span s-live="ticket_status">Open</span>"#,
            &[live_field("status")],
            concat!(
                "invalid FSR live slot configuration between pages/tickets/page.html and pages/tickets/live.rs:\n",
                "- s-live=\"ticket_status\" exists in pages/tickets/page.html but no matching Live field exists in pages/tickets/live.rs\n",
                "- Live field `status` exists in pages/tickets/live.rs but no matching s-live=\"status\" exists in pages/tickets/page.html; add the slot or mark the field with #[pilcrow::allow_unused]",
            ),
        );
    }

    #[test]
    fn live_slot_validation_errors_when_live_field_has_no_template_slot() {
        assert_live_validation_snapshot(
            r#"<span s-live="status">Open</span>"#,
            &[live_field("status"), live_field("priority")],
            concat!(
                "invalid FSR live slot configuration between pages/tickets/page.html and pages/tickets/live.rs:\n",
                "- Live field `priority` exists in pages/tickets/live.rs but no matching s-live=\"priority\" exists in pages/tickets/page.html; add the slot or mark the field with #[pilcrow::allow_unused]",
            ),
        );
    }

    #[test]
    fn live_slot_validation_errors_when_template_has_duplicate_slot_names() {
        assert_live_validation_snapshot(
            r#"<span s-live="status">Open</span><strong s-live="status">Open</strong>"#,
            &[live_field("status")],
            "invalid FSR live slot configuration between pages/tickets/page.html and pages/tickets/live.rs:\n- duplicate s-live slot `status` appears 2 times in pages/tickets/page.html; each FSR slot must be unique so initial HTML baking and live patches target one element clearly",
        );
    }

    #[test]
    fn live_slot_validation_allows_explicitly_unused_live_fields() {
        let mut priority = live_field("priority");
        priority.allow_unused = true;
        validate_live_template_slots(
            r#"<span s-live="status">Open</span>"#,
            &[live_field("status"), priority],
            Path::new("pages/tickets/page.html"),
            Path::new("pages/tickets/live.rs"),
        )
        .expect("allow_unused field should not require an s-live slot");
    }

    #[test]
    fn live_struct_defaults_apply_to_all_live_prop_fields() {
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            #[pilcrow::live(
                patch_debounce = 30,
                depends_on = dep!(tickets, id, params.id)
            )]
            pub struct Live {
                pub ticket_status: LiveProp<String>,
                pub ticket_priority: LiveProp<String>,
            }
            ",
        );

        let route_params = vec!["id".to_string()];
        let (source, _) = process_live_rs(&path, &route_params, None, &Default::default(), "page_test").expect("process live.rs");
        let _ = fs::remove_file(path);

        assert!(source.contains("patch_debounce: ::std::option::Option::Some(30)"));
        // 2 fields × 2 locations (from_row + live_fields) = 4 occurrences.
        assert_eq!(source.matches("\"tickets:id={}\"").count(), 4);
        assert!(source.contains("_params.get(\"id\")"));
        assert!(!source.contains("# [pilcrow :: live"));
    }

    #[test]
    fn live_field_attributes_override_struct_defaults() {
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            #[pilcrow::live(
                patch_debounce = 30,
                depends_on = dep!(tickets, id, params.id)
            )]
            pub struct Live {
                #[pilcrow::patch_debounce(3)]
                #[pilcrow::depends_on(dep!(ticket_priorities, ticket_id, params.id))]
                pub ticket_priority: LiveProp<String>,
                pub ticket_status: LiveProp<String>,
            }
            ",
        );

        let route_params = vec!["id".to_string()];
        let (source, _) = process_live_rs(&path, &route_params, None, &Default::default(), "page_test").expect("process live.rs");
        let _ = fs::remove_file(path);

        assert!(source.contains("ticket_priority: ::pilcrow_runtime::fsr::LiveProp"));
        assert!(source.contains("patch_debounce: ::std::option::Option::Some(3)"));
        assert!(source.contains("\"ticket_priorities:ticket_id={}\""));
        assert!(source.contains("ticket_status: ::pilcrow_runtime::fsr::LiveProp"));
        assert!(source.contains("patch_debounce: ::std::option::Option::Some(30)"));
        assert!(source.contains("\"tickets:id={}\""));
        assert!(!source.contains("# [pilcrow :: depends_on"));
    }

    #[test]
    fn depends_on_route_struct_default_expands_to_route_param_dependency() {
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            #[pilcrow::depends_on_route(tickets, id)]
            pub struct Live {
                pub ticket_status: LiveProp<String>,
                pub ticket_priority: LiveProp<String>,
            }
            ",
        );

        let route_params = vec!["id".to_string()];
        let (source, _) = process_live_rs(&path, &route_params, None, &Default::default(), "page_test").expect("process live.rs");
        let _ = fs::remove_file(path);

        // 2 fields × 2 locations (from_row + live_fields) = 4 occurrences.
        assert_eq!(source.matches("\"tickets:id={}\"").count(), 4);
        assert!(source.contains("_params.get(\"id\")"));
        assert!(!source.contains("# [pilcrow :: depends_on_route"));
    }

    #[test]
    fn depends_on_route_field_override_beats_struct_default() {
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            #[pilcrow::depends_on_route(tickets, id)]
            pub struct Live {
                #[pilcrow::depends_on_route(ticket_priorities, id)]
                pub ticket_priority: LiveProp<String>,
                pub ticket_status: LiveProp<String>,
            }
            ",
        );

        let route_params = vec!["id".to_string()];
        let (source, _) = process_live_rs(&path, &route_params, None, &Default::default(), "page_test").expect("process live.rs");
        let _ = fs::remove_file(path);

        assert!(source.contains("\"ticket_priorities:id={}\""));
        assert!(source.contains("\"tickets:id={}\""));
    }

    #[test]
    fn depends_on_route_errors_when_route_param_is_missing() {
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            #[pilcrow::depends_on_route(tickets, id)]
            pub struct Live {
                pub ticket_status: LiveProp<String>,
            }
            ",
        );

        let err = match process_live_rs(&path, &[], None, &Default::default(), "page_test") {
            Ok(_) => panic!("missing id param should fail"),
            Err(err) => err,
        };
        let _ = fs::remove_file(path);

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let message = err.to_string();
        assert!(message.contains("depends_on_route(tickets, id)"));
        assert!(message.contains("route param `id` is missing"));
        assert!(message.contains("depends_on(dep!(...))"));
    }

    #[test]
    fn allow_unused_field_attribute_is_extracted_and_stripped() {
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            pub struct Live {
                pub ticket_status: LiveProp<String>,
                #[pilcrow::allow_unused]
                pub audit_note: LiveProp<String>,
            }
            ",
        );

        let (source, fields) = process_live_rs(&path, &[], None, &Default::default(), "page_test").expect("process live.rs");
        let _ = fs::remove_file(path);

        let audit_note = fields
            .iter()
            .find(|field| field.name == "audit_note")
            .expect("audit_note field");
        assert!(audit_note.allow_unused);
        assert!(!source.contains("# [pilcrow :: allow_unused"));
    }

    #[test]
    fn column_attribute_overrides_database_mapping() {
        let path = write_live_rs(
            r#"
            use pilcrow_web::live::*;

            pub struct Live {
                #[pilcrow::column("status")]
                pub ticket_status: LiveProp<String>,
            }
            "#,
        );

        let (source, fields) = process_live_rs(&path, &[], None, &Default::default(), "page_test").expect("process live.rs");
        let _ = fs::remove_file(path);

        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].column_name, Some("status".to_string()));
        assert!(source.contains("row.get(\"status\")"));
        assert!(!source.contains("row.get(\"ticket_status\")"));
        assert!(!source.contains("# [pilcrow :: column"));
    }

    #[test]
    fn column_attribute_mixed_renamed_and_default_fields() {
        let path = write_live_rs(
            r#"
            use pilcrow_web::live::*;

            pub struct Live {
                #[pilcrow::column("status")]
                pub ticket_status: LiveProp<String>,
                pub priority: LiveProp<String>,
            }
            "#,
        );

        let (source, fields) = process_live_rs(&path, &[], None, &Default::default(), "page_test").expect("process live.rs");
        let _ = fs::remove_file(path);

        assert_eq!(fields.len(), 2);

        // Renamed field uses column name for DB lookup.
        assert_eq!(fields[0].name, "ticket_status");
        assert_eq!(fields[0].column_name, Some("status".to_string()));
        assert!(source.contains("row.get(\"status\")"));
        assert!(!source.contains("row.get(\"ticket_status\")"));

        // Non-renamed field falls back to its own name.
        assert_eq!(fields[1].name, "priority");
        assert_eq!(fields[1].column_name, None);
        assert!(source.contains("row.get(\"priority\")"));
    }

    #[test]
    fn auto_dep_key_injected_when_revalidate_secs_is_set() {
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            pub struct Live {
                pub status: LiveProp<String>,
                pub priority: LiveProp<String>,
            }
            ",
        );

        let mut auto_attrs = HashMap::new();
        // Only status gets revalidate; priority does not.
        auto_attrs.insert("status".to_string(), LiveFieldAttr { revalidate_secs: Some(60), depends_on: None });

        let (source, _) = process_live_rs(&path, &[], None, &auto_attrs, "page_tickets").expect("process live.rs");
        let _ = fs::remove_file(path);

        // status → shared synthetic dep key injected in both from_row and live_fields.
        assert_eq!(source.matches("\"page_tickets::__revalidate_60s\"").count(), 2,
            "expected shared synthetic dep key for status in both from_row and live_fields");
        // priority → no auto_attr, so falls back to default dep key.
        assert!(source.contains("\"page_tickets::__revalidate_default\""),
            "priority has no revalidate so should use default dep key");
        assert!(!source.contains("\"page_tickets::status\""),
            "old per-field dep key must not appear");
    }

    #[test]
    fn static_depends_on_prop_attr_wires_dep_key() {
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            pub struct Live {
                pub status: LiveProp<String>,
            }
            ",
        );

        let mut auto_attrs = HashMap::new();
        // #[depends_on(\"prices:updated\")] on the Props field.
        auto_attrs.insert("status".to_string(), LiveFieldAttr {
            revalidate_secs: None,
            depends_on: Some("prices:updated".to_string()),
        });

        let (source, _) = process_live_rs(&path, &[], None, &auto_attrs, "page_tickets").expect("process live.rs");
        let _ = fs::remove_file(path);

        // Both from_row and live_fields should use the static dep key.
        assert_eq!(source.matches("\"prices:updated\"").count(), 2,
            "expected static dep key in both from_row and live_fields");
        assert!(!source.contains("\"page_tickets::status\""),
            "auto dep key must not appear when static depends_on is set");
    }

    #[test]
    fn fields_without_revalidate_use_default_dep_key() {
        // A field with no #[revalidate] and no explicit depends_on should use
        // the __revalidate_default synthetic dep key so it gets the global/24h timer.
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            pub struct Live {
                pub summary: LiveProp<String>,
            }
            ",
        );

        let auto_attrs = HashMap::new(); // no attrs at all

        let (source, _) = process_live_rs(&path, &[], None, &auto_attrs, "page_dash").expect("process live.rs");
        let _ = fs::remove_file(path);

        assert_eq!(source.matches("\"page_dash::__revalidate_default\"").count(), 2,
            "expected default dep key in both from_row and live_fields");
    }

    #[test]
    fn same_interval_on_same_route_produces_shared_dep_key() {
        // Two fields with the same #[revalidate(N)] on the same route must resolve
        // to the SAME synthetic dep key (shared timer, not per-field).
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            pub struct Live {
                pub price: LiveProp<f64>,
                pub market_cap: LiveProp<f64>,
            }
            ",
        );

        let mut auto_attrs = HashMap::new();
        auto_attrs.insert("price".to_string(), LiveFieldAttr { revalidate_secs: Some(10), depends_on: None });
        auto_attrs.insert("market_cap".to_string(), LiveFieldAttr { revalidate_secs: Some(10), depends_on: None });

        let (source, _) = process_live_rs(&path, &[], None, &auto_attrs, "page_market").expect("process live.rs");
        let _ = fs::remove_file(path);

        // Both fields get the same synthetic dep key, appearing 4 times total
        // (once in from_row + once in live_fields, per field = 4).
        assert_eq!(source.matches("\"page_market::__revalidate_10s\"").count(), 4,
            "both fields should share one synthetic dep key");
        assert!(!source.contains("__revalidate_default"),
            "no default key when all fields have explicit revalidate");
    }

    #[test]
    fn different_intervals_produce_distinct_dep_keys() {
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            pub struct Live {
                pub price: LiveProp<f64>,
                pub volume: LiveProp<u64>,
            }
            ",
        );

        let mut auto_attrs = HashMap::new();
        auto_attrs.insert("price".to_string(), LiveFieldAttr { revalidate_secs: Some(10), depends_on: None });
        auto_attrs.insert("volume".to_string(), LiveFieldAttr { revalidate_secs: Some(30), depends_on: None });

        let (source, _) = process_live_rs(&path, &[], None, &auto_attrs, "page_market").expect("process live.rs");
        let _ = fs::remove_file(path);

        assert_eq!(source.matches("\"page_market::__revalidate_10s\"").count(), 2,
            "price gets 10s dep key");
        assert_eq!(source.matches("\"page_market::__revalidate_30s\"").count(), 2,
            "volume gets 30s dep key");
    }

    fn explicit_depends_on_beats_auto_dep_key() {
        let path = write_live_rs(
            "
            use pilcrow_web::live::*;

            pub struct Live {
                #[pilcrow::depends_on(dep!(tickets, id, params.id))]
                pub status: LiveProp<String>,
            }
            ",
        );

        let route_params = vec!["id".to_string()];
        let mut auto_attrs = HashMap::new();
        auto_attrs.insert("status".to_string(), LiveFieldAttr { revalidate_secs: Some(30), depends_on: None });

        let (source, _) = process_live_rs(&path, &route_params, None, &auto_attrs, "page_tickets").expect("process live.rs");
        let _ = fs::remove_file(path);

        // Explicit dep key wins — tickets:id=, NOT page_tickets::status.
        assert!(source.contains("\"tickets:id={}\""), "explicit dep should appear");
        assert!(!source.contains("\"page_tickets::status\""),
            "auto dep key must not override explicit depends_on");
    }
}
