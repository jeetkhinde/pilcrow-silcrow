use super::*;

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
    // Removed (emit build errors): REVALIDATE, MAX_STALE, CACHE_TAGS, CACHE_VARY, STREAMING.
    let mut page_options = PageOptions::default();
    let mut const_remove_indices: Vec<usize> = Vec::new();
    for (index, item) in file.items.iter().enumerate() {
        if let syn::Item::Const(c) = item
            && matches!(c.vis, syn::Visibility::Public(_))
        {
            let value_str = c.expr.to_token_stream().to_string();
            let value = value_str.trim_matches('"').trim_matches('\'');
            if c.ident == "TRAILING_SLASH" {
                page_options.trailing_slash = TrailingSlash::from_label(value);
                const_remove_indices.push(index);
            } else if c.ident == "LAYOUT" {
                page_options.layout = if value.trim_matches('"').trim_matches('\'') == "none" {
                    LayoutOpt::None
                } else {
                    LayoutOpt::Inherit
                };
                const_remove_indices.push(index);
            } else if c.ident == "REVALIDATE" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{source_path}: REVALIDATE is removed. \
                         Use FSR with ScheduledInvalidation instead: register \
                         ScheduledInvalidation::new(\"<dep_key>\", Duration::from_secs(N)) \
                         in WatcherConfig::scheduled_invalidations."
                    ),
                ));
            } else if c.ident == "MAX_STALE" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{source_path}: MAX_STALE is removed along with the ISR cache. \
                         Use FSR LiveProp<T> with promote_after for route-level baking."
                    ),
                ));
            } else if c.ident == "CACHE_TAGS" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{source_path}: CACHE_TAGS is removed. \
                         Use FSR dep keys and FsrStore::invalidate_dep_key() for targeted invalidation."
                    ),
                ));
            } else if c.ident == "CACHE_VARY" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{source_path}: CACHE_VARY is removed along with the ISR cache."
                    ),
                ));
            } else if c.ident == "PRERENDER" {
                let is_prerender = value_str.trim() == "true";
                page_options.ssg.prerender = is_prerender;
                const_remove_indices.push(index);
            } else if c.ident == "PROMOTE_AFTER" {
                if let Ok(v) = parse_u64_const(&c.expr) {
                    if v > u32::MAX as u64 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "{source_path}: PROMOTE_AFTER value {v} overflows u32 (max {}). \
                                 Use a value in 0..={}.",
                                u32::MAX,
                                u32::MAX,
                            ),
                        ));
                    }
                    page_options.fsr.promote_after = Some(v as u32);
                }
                const_remove_indices.push(index);
            } else if c.ident == "STREAMING" {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{source_path}: STREAMING is removed. \
                         Use FSR LiveProp<T> for field-level live updates, or plain SSR."
                    ),
                ));
            } else if c.ident == "FSR_JSON" {
                page_options.fsr.json = value_str.trim() == "true";
                const_remove_indices.push(index);
            }
        }
    }
    // Remove in reverse order to preserve indices.
    for idx in const_remove_indices.into_iter().rev() {
        file.items.remove(idx);
    }

    let mut props_indices = Vec::new();
    for (index, item) in file.items.iter().enumerate() {
        if let syn::Item::Struct(item_struct) = item
            && item_struct.ident == "Props"
        {
            if !matches!(item_struct.vis, syn::Visibility::Public(_)) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("frontmatter in {source_path} must declare `pub struct Props`"),
                ));
            }
            props_indices.push(index);
        }
    }

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
        if let syn::Item::Fn(f) = item {
            if f.sig.ident == "load" {
                Some(detect_load_signature(&f.sig))
            } else {
                None
            }
        } else {
            None
        }
    });

    // Detect `entries()` — marks a dynamic SSG route with an explicit param list.
    // Required signature: `pub async fn entries() -> Vec<...>` (no parameters).
    let has_entries_fn = file.items.iter().any(|item| {
        if let syn::Item::Fn(f) = item {
            f.sig.ident == "entries"
                && f.sig.asyncness.is_some()
                && f.sig.inputs.is_empty()
                && matches!(f.vis, syn::Visibility::Public(_))
        } else {
            false
        }
    });
    if has_entries_fn {
        page_options.ssg.has_entries_fn = true;
    }

    // PRERENDER = true on a FSR route is equivalent to promote_after = 0, but only
    // when PROMOTE_AFTER was not declared explicitly (explicit value always wins).
    if page_options.ssg.prerender && page_options.fsr.promote_after.is_none() {
        page_options.fsr.promote_after = Some(0);
    }

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
        let wants_req = f.sig.inputs.iter().any(|arg| {
            if let syn::FnArg::Typed(pat) = arg {
                type_last_ident(&pat.ty)
                    .map(|id| id == "Req")
                    .unwrap_or(false)
            } else {
                false
            }
        });

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
        if let syn::Item::Impl(impl_block) = item
            && let Some((_, path, _)) = &impl_block.trait_
        {
            return path.segments.last().is_some_and(|s| s.ident == "Default");
        }
        false
    });

    for item in &mut file.items {
        match item {
            syn::Item::Struct(item_struct) => ensure_serialize_derive(&mut item_struct.attrs),
            syn::Item::Enum(item_enum) => ensure_serialize_derive(&mut item_enum.attrs),
            syn::Item::Union(item_union) => ensure_serialize_derive(&mut item_union.attrs),
            _ => {}
        }
    }

    // If the user didn't declare `pub struct Props`, synthesize a unit struct.
    let props_index = if let Some(idx) = props_indices.first().copied() {
        idx
    } else {
        file.items.push(parse_quote!(
            pub struct Props;
        ));
        file.items.len() - 1
    };

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

    // Detect `LiveProp<T>` fields.
    let live_fields: Vec<String> = own_syn_fields
        .iter()
        .filter_map(|f| {
            if let Some(ident) = &f.ident
                && type_last_ident(&f.ty)
                    .map(|id| id == "LiveProp")
                    .unwrap_or(false)
            {
                return Some(ident.to_string());
            }
            None
        })
        .collect();

    // Rewrite {{ live_field }} → <span :text="live_field">{{ live_field }}</span>
    // for Dom/DomAndStore fields so Silcrow can patch them via SSE.
    let live_template = if live_fields.is_empty() {
        std::borrow::Cow::Borrowed(template_source)
    } else {
        std::borrow::Cow::Owned(crate::templating::compiler::inject_live_text_spans(
            template_source,
            &live_fields,
        ))
    };
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
        if let syn::Item::Use(u) = item {
            let s = u.to_token_stream().to_string();
            // ":: Req" matches `use pilcrow_web::Req`, "{ Req" matches grouped imports.
            s.contains(":: Req") || s.contains("{ Req")
        } else {
            false
        }
    });

    let mut out = String::new();
    out.push_str("#[allow(unused_imports)]\n");
    out.push_str("use pilcrow_web::pilcrow_client::PilcrowClient;\n");
    out.push_str("#[allow(unused_imports)]\n");
    out.push_str("use pilcrow_web::AppResult;\n");
    out.push_str("#[allow(unused_imports)]\n");
    out.push_str("use pilcrow_web::AppResult as PilcrowResult;\n");
    if !has_req_import {
        out.push_str("#[allow(unused_imports)]\n");
        out.push_str("use pilcrow_web::Req;\n");
    }
    out.push_str("#[allow(unused_imports)]\n");
    out.push_str("use pilcrow_web::ActionResult;\n");
    out.push_str("#[allow(unused_imports)]\n");
    out.push_str("use pilcrow_web::redirect;\n");
    out.push_str("#[allow(unused_imports)]\n");
    out.push_str("use pilcrow_web::{ok, ActionResultExt, ResponseExt, ToastLevel};\n");
    // Make `fragments::prefix::name::render(props)` available without an explicit import.
    // Not injected for ui/ components since they can't call fragments directly.
    if !in_ui {
        out.push_str("#[allow(unused_imports)]\n");
        out.push_str("use super::fragments;\n");
    }
    for item in file.items {
        out.push_str(&item.into_token_stream().to_string());
        out.push('\n');
    }
    Ok(InstrumentedFrontmatter {
        source: out,
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

/// Inspect a `load()` `fn` signature to decide how the generated handler
/// should call it: sync vs async, infallible vs `Result`, with/without
/// `PilcrowClient` injection.
pub fn detect_load_signature(sig: &syn::Signature) -> LoadSignature {
    let is_async = sig.asyncness.is_some();

    let returns_result = match &sig.output {
        syn::ReturnType::Default => false,
        syn::ReturnType::Type(_, ty) => type_last_ident(ty)
            .map(|ident| ident == "Result" || ident == "AppResult" || ident == "PilcrowResult")
            .unwrap_or(false),
    };

    let wants_client = sig.inputs.iter().any(|arg| {
        if let syn::FnArg::Typed(pat) = arg {
            type_last_ident(&pat.ty)
                .map(|ident| ident == "PilcrowClient")
                .unwrap_or(false)
        } else {
            false
        }
    });

    let wants_req = sig.inputs.iter().any(|arg| {
        if let syn::FnArg::Typed(pat) = arg {
            type_last_ident(&pat.ty)
                .map(|ident| ident == "Req")
                .unwrap_or(false)
        } else {
            false
        }
    });

    let wants_page = sig.inputs.iter().any(|arg| {
        if let syn::FnArg::Typed(pat) = arg {
            type_last_ident(&pat.ty)
                .map(|ident| ident == "Page")
                .unwrap_or(false)
        } else {
            false
        }
    });

    let wants_live = sig.inputs.iter().any(|arg| {
        if let syn::FnArg::Typed(pat) = arg {
            type_last_ident(&pat.ty)
                .map(|ident| ident == "Live")
                .unwrap_or(false)
        } else {
            false
        }
    });

    LoadSignature {
        is_async,
        returns_result,
        wants_client,
        wants_req,
        wants_page,
        wants_live,
    }
}

pub fn inject_props_attrs(props: &mut syn::ItemStruct, template_source: &str) {
    let mut missing_derives = Vec::<syn::Path>::new();
    if !has_derive_trait(&props.attrs, &["askama::Template", "Template"]) {
        missing_derives.push(parse_quote!(askama::Template));
    }
    if !has_derive_trait(&props.attrs, &["serde::Serialize", "Serialize"]) {
        missing_derives.push(parse_quote!(serde::Serialize));
    }

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
    // Detect field name collisions between layout Props and page Props up front.
    let layout_names = named_field_names(extra_fields);
    let page_names = named_field_names(own_fields);
    for name in &page_names {
        if layout_names.contains(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Props field `{name}` is defined in both the layout and the page — \
                     rename one of them to avoid a collision in __MergedProps"
                ),
            ));
        }
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

    for f in extra_fields {
        named_fields.named.push(f.clone());
    }
    for f in own_fields {
        named_fields.named.push(f.clone());
    }

    Ok(item)
}

pub fn ensure_serialize_derive(attrs: &mut Vec<syn::Attribute>) {
    if has_derive_trait(attrs, &["serde::Serialize", "Serialize"]) {
        return;
    }
    attrs.push(parse_quote!(#[derive(serde::Serialize)]));
}

pub fn has_derive_trait(attrs: &[syn::Attribute], candidates: &[&str]) -> bool {
    let normalized_candidates = candidates
        .iter()
        .map(|candidate| normalize_derive_path(candidate))
        .collect::<Vec<_>>();

    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }

        let tokens = attr.meta.to_token_stream().to_string();
        let normalized_tokens = normalize_derive_path(&tokens);
        if normalized_candidates
            .iter()
            .any(|candidate| normalized_tokens.contains(candidate))
        {
            return true;
        }
    }

    false
}

pub fn normalize_derive_path(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
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

