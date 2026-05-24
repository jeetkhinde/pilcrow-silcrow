use super::*;
use crate::templating::page_options::LiveFieldAttr;

/// `extra_fields` contains the named fields from a layout's Props struct when the layout
/// declares `load()`. When non-empty, a `__MergedProps` struct is synthesized in the
/// page module that combines the layout fields with the page's own fields.  The original
/// `Props` struct is kept as-is so the page's own `load()` can still return it.
pub fn instrument_frontmatter(
    rust_frontmatter: &str,
    template_source: &str,
    source_path: &str,
    extra_fields: &[syn::Field],
    is_fragment: bool,
) -> io::Result<InstrumentedFrontmatter> {
    let mut file = syn::parse_file(rust_frontmatter).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse rust frontmatter in {source_path}: {err}"),
        )
    })?;

    // Parse and strip framework-reserved `pub const` declarations before other processing.
    // Handled: TRAILING_SLASH, LAYOUT, PRERENDER, PROMOTE_AFTER, FSR_JSON.
    let mut page_options = PageOptions::default();
    let mut promote_after_err: Option<io::Error> = None;
    file.items.retain(|item| {
        let syn::Item::Const(c) = item else { return true };
        if !matches!(c.vis, syn::Visibility::Public(_)) { return true; }
        let value_str = c.expr.to_token_stream().to_string();
        let value = value_str.trim_matches('"').trim_matches('\'');
        let ident = c.ident.to_string();
        match ident.as_str() {
            "TRAILING_SLASH" => { page_options.trailing_slash = TrailingSlash::from_label(value); false }
            "LAYOUT" => {
                page_options.layout = if value == "none" { LayoutOpt::None } else { LayoutOpt::Inherit };
                false
            }
            "PROMOTE_AFTER" => {
                if let Ok(v) = parse_u64_const(&c.expr) {
                    if v > u32::MAX as u64 {
                        promote_after_err = Some(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "{source_path}: PROMOTE_AFTER value {v} overflows u32 (max {}). \
                                 Use a value in 0..={}.",
                                u32::MAX,
                                u32::MAX,
                            ),
                        ));
                    } else {
                        page_options.fsr.promote_after = Some(v as u32);
                    }
                }
                false
            }
            "FSR_JSON" => { page_options.fsr.json = value_str.trim() == "true"; false }
            _ => true,
        }
    });
    if let Some(err) = promote_after_err { return Err(err); }

    // Validate that any `Props` struct present is public.
    if file.items.iter().any(|item| {
        let syn::Item::Struct(s) = item else { return false };
        s.ident == "Props" && !matches!(s.vis, syn::Visibility::Public(_))
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frontmatter in {source_path} must declare `pub struct Props`"),
        ));
    }
    let props_indices: Vec<usize> = file.items.iter().enumerate()
        .filter_map(|(i, item)| {
            let syn::Item::Struct(s) = item else { return None };
            (s.ident == "Props").then_some(i)
        })
        .collect();
    if props_indices.len() > 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "frontmatter in {source_path} declares multiple `Props` structs; expected exactly one"
            ),
        ));
    }

    // Detect `load()` function signature. Absent means the page/component is static.
    let load_signature = file.items.iter().find_map(|item| {
        let syn::Item::Fn(f) = item else { return None };
        (f.sig.ident == "load").then(|| detect_load_signature(&f.sig))
    });

    // Detect `entries()` — marks a dynamic SSG route with an explicit param list.
    // Required signature: `pub async fn entries() -> Vec<...>` (no parameters).
    page_options.ssg.has_entries_fn = file.items.iter().any(|item| {
        let syn::Item::Fn(f) = item else { return false };
        f.sig.ident == "entries"
            && f.sig.asyncness.is_some()
            && f.sig.inputs.is_empty()
            && matches!(f.vis, syn::Visibility::Public(_))
    });

    // Discover named action handlers. An action is any `pub` fn in a page's
    // code-behind with an `ActionResult`-shaped return. The fn name is the URL
    // key (`?/<name>`). `load` is excluded — it is the GET handler.
    let in_pages = source_path.contains("/pages/");
    let in_ui = source_path.contains("/ui/");
    let is_layout = source_path
        .rsplit('/')
        .next()
        .and_then(|name| name.split('.').next())
        .is_some_and(|name| name == "_layout");
    let can_own_actions = in_pages || is_fragment;
    let mut actions: Vec<ActionFn> = Vec::new();

    for item in &file.items {
        let syn::Item::Fn(f) = item else { continue };
        let name = f.sig.ident.to_string();
        if name == "load" {
            continue;
        }
        if !matches!(f.vis, syn::Visibility::Public(_)) {
            continue;
        }

        // Helpers use any non-Result return type; only action-shaped fns are considered.
        let returns_result = match &f.sig.output {
            syn::ReturnType::Default => continue,
            syn::ReturnType::Type(_, ty) => {
                let Some(ident) = type_last_ident(ty) else {
                    continue;
                };
                if ident != "Result" && ident != "ActionResult" {
                    continue;
                }
                true
            }
        };

        let is_async = f.sig.asyncness.is_some();
        let wants_req = sig_wants(&f.sig, "Req");

        if in_ui {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "action handler `{name}` is not allowed in `{source_path}`; \
                     ui components cannot handle form actions — move it to a page."
                ),
            ));
        }
        if is_layout {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "action handler `{name}` is not allowed in `{source_path}`; \
                     layouts cannot handle form actions — move it to a page or fragment."
                ),
            ));
        }

        // Layouts do not own actions. Skip silently so users can still define
        // helper fns there if their signature happens to match.
        if !can_own_actions {
            continue;
        }

        if !is_async {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "action `{name}` in `{source_path}` must be declared `async`.\n\
                         Required signature: pub async fn {name}(req: Req) -> ActionResult"
                ),
            ));
        }
        if !wants_req {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "action `{name}` in `{source_path}` must take `req: Req` as a parameter.\n\
                     Required signature: pub async fn {name}(req: Req) -> ActionResult"
                ),
            ));
        }

        actions.push(ActionFn {
            name,
            is_async,
            returns_result,
            wants_req,
        });
    }

    // UI components are Props-only: they display data, they don't fetch it.
    // Layouts CAN have load() — they provide shared data to every page they wrap.
    if in_ui && load_signature.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "`load()` is not allowed in `{source_path}`; \
                 ui components are Props-only. \
                 Fetch data in a page (`pages/`) or layout (`layouts/`) and pass it via Props."
            ),
        ));
    }

    // Pages and layouts must use a canonical load() signature so that
    // response modifiers (toasts, headers, cookies) are always applied.
    if let Some(ref sig) = load_signature
        && (source_path.contains("/pages/") || is_fragment)
    {
        if !sig.is_async {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "`load()` in `{source_path}` must be declared `async`.\n\
                         Required signature: pub async fn load(req: Req) -> AppResult<Props> or pub async fn load(ctx: Page) -> AppResult<Props>"
                ),
            ));
        }
        if !sig.returns_result {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "`load()` in `{source_path}` must return `AppResult<Props>`.\n\
                         Required signature: pub async fn load(req: Req) -> AppResult<Props> or pub async fn load(ctx: Page) -> AppResult<Props>"
                ),
            ));
        }
        if !sig.wants_req && !sig.wants_page {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "`load()` in `{source_path}` must take `req: Req` or `ctx: Page` as a parameter.\n\
                         Required signature: pub async fn load(req: Req) -> AppResult<Props> or pub async fn load(ctx: Page) -> AppResult<Props>"
                ),
            ));
        }
    }

    let has_manual_default = file.items.iter().any(|item| {
        let syn::Item::Impl(impl_block) = item else { return false };
        impl_block.trait_.as_ref()
            .and_then(|(_, path, _)| path.segments.last())
            .is_some_and(|s| s.ident == "Default")
    });

    file.items.iter_mut().for_each(|item| match item {
        syn::Item::Struct(s) => ensure_serialize_derive(&mut s.attrs),
        syn::Item::Enum(e) => ensure_serialize_derive(&mut e.attrs),
        syn::Item::Union(u) => ensure_serialize_derive(&mut u.attrs),
        _ => {}
    });

    // If the user didn't declare `pub struct Props`, synthesize a unit struct.
    let props_index = props_indices.first().copied().unwrap_or_else(|| {
        file.items.push(parse_quote!(pub struct Props;));
        file.items.len() - 1
    });

    // Detect a `live()` fn or `LiveProp` struct — must be done before the mutable borrow.
    let has_live_fn = file.items.iter().any(|item| match item {
        syn::Item::Fn(f) => f.sig.ident == "live",
        syn::Item::Struct(s) => s.ident == "LiveProp",
        _ => false,
    });

    let props_item = file
        .items
        .get_mut(props_index)
        .expect("validated props index should exist");

    let syn::Item::Struct(props_struct) = props_item else {
        unreachable!("validated Props item is not struct")
    };

    // Extract the page's own named fields before any modification.
    let own_syn_fields = extract_named_fields(props_struct);

    // Parse and strip `#[pilcrow::live(...)]` from LiveProp<T> Props fields.
    // Collects per-field revalidate_secs for FSR codegen. Errors on unknown keys.
    let mut live_field_err: Option<io::Error> = None;
    if let syn::Fields::Named(ref mut named) = props_struct.fields {
        for field in &mut named.named {
            let Some(ident) = &field.ident else { continue };
            if !type_last_ident(&field.ty).is_some_and(|id| id == "LiveProp") {
                continue;
            }
            let field_name = ident.to_string();
            let mut live_attr = LiveFieldAttr::default();
            let mut found = false;
            field.attrs.retain(|a| {
                if !is_pilcrow_live_field_attr(a) {
                    return true;
                }
                found = true;
                if let Ok(items) = a.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for item in items {
                        let syn::Meta::NameValue(nv) = item else { continue };
                        if nv.path.is_ident("revalidate") {
                            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(n), .. }) = &nv.value {
                                live_attr.revalidate_secs = n.base10_parse::<u64>().ok();
                            }
                        } else {
                            let key = nv.path.segments.last()
                                .map(|s| s.ident.to_string())
                                .unwrap_or_default();
                            live_field_err = Some(io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!(
                                    "`{source_path}`: unknown key `{key}` in \
                                     `#[pilcrow::live(...)]` on field `{field_name}`. \
                                     Only `revalidate` is accepted on Props fields."
                                ),
                            ));
                        }
                    }
                }
                false // strip the attribute
            });
            if found {
                page_options.fsr.live_field_attrs.insert(field_name, live_attr);
            }
        }
    }
    if let Some(err) = live_field_err {
        return Err(err);
    }

    // Detect `LiveProp<T>` fields.
    let live_fields: Vec<String> = own_syn_fields.iter()
        .filter_map(|f| {
            let ident = f.ident.as_ref()?;
            type_last_ident(&f.ty).filter(|id| *id == "LiveProp")?;
            Some(ident.to_string())
        })
        .collect();

    // Rewrite {{ live_field }} → <span :text="live_field">{{ live_field }}</span>
    // for Dom/DomAndStore fields so Silcrow can patch them via SSE.
    let live_template = (!live_fields.is_empty())
        .then(|| crate::templating::compiler::inject_live_text_spans(template_source, &live_fields))
        .map(std::borrow::Cow::Owned)
        .unwrap_or(std::borrow::Cow::Borrowed(template_source));
    let template_source = live_template.as_ref();

    if extra_fields.is_empty() {
        // Normal path: Props is used directly for template rendering.
        // Inject Default derive for static pages (no load function).
        if load_signature.is_none()
            && !has_manual_default
            && !has_derive_trait(&props_struct.attrs, &["Default"])
        {
            props_struct.attrs.push(parse_quote!(#[derive(Default)]));
        }
        inject_props_attrs(props_struct, template_source);
    } else {
        // Layout-merge path: Props keeps the page's own fields for load() return.
        // A separate __MergedProps struct (layout fields + page fields) is emitted
        // and used by the render function and the generated handler.
        // serde::Serialize was already added by the loop above; askama::Template
        // goes on __MergedProps only.
        let merged = make_merged_props_struct(extra_fields, &own_syn_fields, template_source)?;
        file.items.push(syn::Item::Struct(merged));
    }

    // Check whether the user's code already imports Req to avoid E0252.
    let has_req_import = file.items.iter().any(|item| {
        let syn::Item::Use(u) = item else { return false };
        let s = u.to_token_stream().to_string();
        // ":: Req" matches `use pilcrow_web::Req`, "{ Req" matches grouped imports.
        s.contains(":: Req") || s.contains("{ Req")
    });

    let preamble: String = [
        "#[allow(unused_imports)]\nuse pilcrow_web::pilcrow_client::PilcrowClient;\n",
        "#[allow(unused_imports)]\nuse pilcrow_web::AppResult;\n",
        "#[allow(unused_imports)]\nuse pilcrow_web::AppResult as PilcrowResult;\n",
    ]
    .into_iter()
    .chain((!has_req_import).then_some("#[allow(unused_imports)]\nuse pilcrow_web::Req;\n"))
    .chain([
        "#[allow(unused_imports)]\nuse pilcrow_web::ActionResult;\n",
        "#[allow(unused_imports)]\nuse pilcrow_web::redirect;\n",
        "#[allow(unused_imports)]\nuse pilcrow_web::{ok, ActionResultExt, ResponseExt, ToastLevel};\n",
    ])
    .chain((!in_ui).then_some("#[allow(unused_imports)]\nuse super::fragments;\n"))
    .collect();

    let body: String = file.items.into_iter()
        .map(|item| item.into_token_stream().to_string() + "\n")
        .collect();

    Ok(InstrumentedFrontmatter {
        source: preamble + &body,
        load_signature,
        own_syn_fields,
        actions,
        page_options,
        live_fields,
        has_live_fn,
        fsr_live_source: None, // Set by templates.rs after calling process_live_rs
        fsr_live_fields: vec![], // Set by templates.rs
    })
}

/// Returns true if any typed argument in `sig` has a type whose last path segment matches `type_name`.
fn sig_wants(sig: &syn::Signature, type_name: &str) -> bool {
    sig.inputs.iter().any(|arg| {
        let syn::FnArg::Typed(pat) = arg else { return false };
        type_last_ident(&pat.ty).map(|id| id == type_name).unwrap_or(false)
    })
}

/// Inspect a `load()` `fn` signature to decide how the generated handler
/// should call it: sync vs async, infallible vs `Result`, with/without
/// `PilcrowClient` injection.
pub fn detect_load_signature(sig: &syn::Signature) -> LoadSignature {
    LoadSignature {
        is_async: sig.asyncness.is_some(),
        returns_result: match &sig.output {
            syn::ReturnType::Default => false,
            syn::ReturnType::Type(_, ty) => type_last_ident(ty)
                .map(|id| id == "Result" || id == "AppResult" || id == "PilcrowResult")
                .unwrap_or(false),
        },
        wants_client: sig_wants(sig, "PilcrowClient"),
        wants_req:    sig_wants(sig, "Req"),
        wants_page:   sig_wants(sig, "Page"),
        wants_live:   sig_wants(sig, "Live"),
    }
}

pub fn inject_props_attrs(props: &mut syn::ItemStruct, template_source: &str) {
    let all: [(&[&str], syn::Path); 2] = [
        (&["askama::Template", "Template"], parse_quote!(askama::Template)),
        (&["serde::Serialize", "Serialize"], parse_quote!(serde::Serialize)),
    ];
    let missing_derives: Vec<syn::Path> = all
        .into_iter()
        .filter(|(candidates, _)| !has_derive_trait(&props.attrs, candidates))
        .map(|(_, path)| path)
        .collect();

    if !missing_derives.is_empty() {
        props
            .attrs
            .push(parse_quote!(#[derive(#(#missing_derives),*)]));
    }

    if !props
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("template"))
    {
        let template_lit = syn::LitStr::new(template_source, Span::call_site());
        props
            .attrs
            .push(parse_quote!(#[template(source = #template_lit, ext = "html")]));
    }
}

/// Extract the named fields from a struct's field list.
pub fn extract_named_fields(item_struct: &syn::ItemStruct) -> Vec<syn::Field> {
    match &item_struct.fields {
        syn::Fields::Named(named) => named.named.iter().cloned().collect(),
        _ => vec![],
    }
}

/// Build the `__MergedProps` struct used when a layout's `load()` contributes fields.
///
/// Fields order: layout fields first, then the page's own fields.
/// Derives `askama::Template` and `serde::Serialize`; attaches the Askama template source.
/// Returns an error if any field name appears in both `extra_fields` and `own_fields`.
pub fn make_merged_props_struct(
    extra_fields: &[syn::Field],
    own_fields: &[syn::Field],
    template_source: &str,
) -> io::Result<syn::ItemStruct> {
    let layout_names = named_field_names(extra_fields);
    let page_names = named_field_names(own_fields);
    if let Some(name) = page_names.iter().find(|n| layout_names.contains(*n)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Props field `{name}` is defined in both the layout and the page — \
                 rename one of them to avoid a collision in __MergedProps"
            ),
        ));
    }

    let template_lit = syn::LitStr::new(template_source, Span::call_site());

    // Start with an empty named struct and populate the fields list.
    let mut item: syn::ItemStruct = parse_quote! {
        #[derive(askama::Template, serde::Serialize)]
        #[template(source = #template_lit, ext = "html")]
        pub struct __MergedProps {}
    };

    let syn::Fields::Named(ref mut named_fields) = item.fields else {
        unreachable!("just parsed a named struct")
    };
    named_fields.named.extend(extra_fields.iter().cloned());
    named_fields.named.extend(own_fields.iter().cloned());

    Ok(item)
}

pub fn ensure_serialize_derive(attrs: &mut Vec<syn::Attribute>) {
    if has_derive_trait(attrs, &["serde::Serialize", "Serialize"]) {
        return;
    }
    attrs.push(parse_quote!(#[derive(serde::Serialize)]));
}

pub fn has_derive_trait(attrs: &[syn::Attribute], candidates: &[&str]) -> bool {
    let normalized: Vec<_> = candidates.iter().map(|c| normalize_derive_path(c)).collect();
    attrs.iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .any(|attr| {
            let tokens = normalize_derive_path(&attr.meta.to_token_stream().to_string());
            normalized.iter().any(|c| tokens.contains(c))
        })
}

pub fn normalize_derive_path(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
}

/// Returns true if the attribute is `#[pilcrow::live(...)]` (field-level, not struct-level).
fn is_pilcrow_live_field_attr(attr: &syn::Attribute) -> bool {
    let segs: Vec<_> = attr.path().segments.iter().map(|s| s.ident.to_string()).collect();
    segs == ["pilcrow", "live"]
}

/// Parse a `u64` literal from a `pub const X: u64 = N;` expression.
fn parse_u64_const(expr: &syn::Expr) -> Result<u64, ()> {
    if let syn::Expr::Lit(lit_expr) = expr
        && let syn::Lit::Int(lit_int) = &lit_expr.lit
    {
        return lit_int.base10_parse::<u64>().map_err(|_| ());
    }
    Err(())
}
