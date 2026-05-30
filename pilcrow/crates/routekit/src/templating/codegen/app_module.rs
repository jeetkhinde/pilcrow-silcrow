use super::*;

// ── App module codegen (auto-wired router) ──────────────────

/// Inline JS injected into every FSR-enabled page to open an SSE connection and
/// patch the DOM when live-prop updates arrive.
///
/// Defined once here so all three use-sites (two codegen paths + the test helper)
/// are guaranteed to stay in sync.
const FSR_PATCH_SCRIPT: &str = "(function(){window.__fsr_route=window.location.pathname;function __fsr_slots(){return Array.from(document.querySelectorAll('[s-live]')).map(function(e){return e.getAttribute('s-live');}).filter(Boolean).join(',');}function __fsr_patch(d){try{Object.keys(d).forEach(function(k){var v=d[k];if(v!==null&&typeof v==='object'&&!Array.isArray(v)){if(window.Silcrow&&window.Silcrow.publish){window.Silcrow.publish('fsr.'+k,v);}}else{document.querySelectorAll('[s-live=\"'+k+'\"]').forEach(function(n){n.textContent=v==null?'':String(v);});}});}catch(x){}}function __fsr_resync(){fetch('/__pilcrow/fsr/snapshot?route='+encodeURIComponent(window.__fsr_route)+'&slots='+encodeURIComponent(__fsr_slots())).then(function(r){return r.json();}).then(__fsr_patch).catch(function(){});}function __fsr_connect(){if(window.__fsr_es){window.__fsr_es.close();}var url='/__pilcrow/fsr?route='+encodeURIComponent(window.__fsr_route)+'&slots='+encodeURIComponent(__fsr_slots());window.__fsr_es=new EventSource(url);window.__fsr_es.addEventListener('fsr',function(e){try{__fsr_patch(JSON.parse(e.data));}catch(x){}});window.__fsr_es.addEventListener('fsr-resync',function(){__fsr_resync();});}__fsr_connect();if(!window.__fsr_nav_bound){window.__fsr_nav_bound=true;document.addEventListener('silcrow:navigate',function(e){if(e.detail&&e.detail.url)window.__fsr_route=new URL(e.detail.url,location.origin).pathname;if(window.__fsr_es){window.__fsr_es.close();window.__fsr_es=null;}});document.addEventListener('silcrow:load',function(){window.__fsr_route=window.location.pathname;var s=__fsr_slots();if(s)__fsr_connect();});}})()";

/// All route-map references needed during app-module codegen.
/// Bundled into a single struct to keep function argument counts manageable.
pub struct AppCodegenMaps<'a> {
    pub load_map: &'a HashMap<String, Option<LoadSignature>>,
    pub layout_fields_map: &'a HashMap<String, LayoutFieldsInfo>,
    pub error_module_for_page: &'a HashMap<String, String>,
    pub not_found_module: Option<&'a str>,
    pub loading_module_for_page: &'a HashMap<String, String>,
    pub action_map: &'a HashMap<String, Vec<ActionFn>>,
    pub page_options_map: &'a HashMap<String, PageOptions>,
    pub live_fields_map: &'a HashMap<String, Vec<String>>,
    pub has_live_fn_map: &'a HashMap<String, bool>,
    pub fsr_live_source_map: &'a HashMap<String, String>,
    pub fsr_live_fields_map: &'a HashMap<String, Vec<String>>,
    /// Set of FSR module names that have fields needing the global/default revalidation timer.
    pub fsr_default_revalidate_symbols: &'a HashSet<String>,
    /// Maps page module symbol → layout IDs for X-PS-Present comparison.
    pub layout_chain_ids_map: &'a HashMap<String, Vec<String>>,
    /// Maps page module symbol → data-ps-slot route pattern.
    pub page_slot_map: &'a HashMap<String, String>,
}

/// Emit the X-PS-Present check that sets `__is_ps_fragment: bool` and extracts the fragment.
///
/// Emits:
/// ```rust
/// const __PS_LAYOUT_CHAIN: &[&str] = &["/", "/tickets"];
/// const __PS_SLOT: &str = "/tickets/:id";
/// let html = if __is_ps_fragment {
///     ::pilcrow_web::extract_ps_fragment(&html, __PS_SLOT)
/// } else { html };
/// let __is_ps_fragment = ...;
/// ```
fn emit_ps_fragment_check(layout_chain: &[String], slot: &str) -> String {
    let chain_literal = layout_chain
        .iter()
        .map(|id| format!("\"{}\"", id))
        .collect::<Vec<_>>()
        .join(", ");
    let slot_lit = rust_string(slot);
    let mut s = String::new();
    let _ = writeln!(
        s,
        "            const __PS_LAYOUT_CHAIN: &[&str] = &[{chain_literal}];"
    );
    let _ = writeln!(s, "            const __PS_SLOT: &str = {slot_lit};");
    s.push_str("            let __is_ps_fragment = req.headers.get(\"x-ps-present\")\n");
    s.push_str("                .and_then(|v| v.to_str().ok())\n");
    s.push_str("                .map(|present| { let __layouts: ::std::collections::HashSet<&str> = present.split(',').map(str::trim).collect(); __PS_LAYOUT_CHAIN.iter().all(|p| __layouts.contains(p)) })\n");
    s.push_str("                .unwrap_or(false);\n");
    s.push_str("            let html = if __is_ps_fragment {\n");
    s.push_str("                ::pilcrow_web::extract_ps_fragment(&html, __PS_SLOT)\n");
    s.push_str("            } else {\n");
    s.push_str("                html\n");
    s.push_str("            };\n");
    s
}

/// Emit the if/else block that:
/// - When `__is_ps_fragment`: returns fragment response with x-ps-fragment=1 content-type,
///   applying __resp_handle when `needs_req` is true.
/// - Otherwise: returns full-page Html response, applying __resp_handle when `needs_req` is true.
///
/// Callers must have already emitted `emit_ps_fragment_check(...)` before this.
fn emit_ps_response(needs_req: bool) -> String {
    let mut s = String::new();
    s.push_str("            if __is_ps_fragment {\n");
    if needs_req {
        s.push_str("                let mut __response = (::pilcrow_web::StatusCode::OK, [(::pilcrow_web::axum::http::header::CONTENT_TYPE, \"text/html; x-ps-fragment=1\")], html).into_response();\n");
        s.push_str("                __resp_handle.apply_to(&mut __response);\n");
        s.push_str("                __response\n");
    } else {
        s.push_str("                (::pilcrow_web::StatusCode::OK, [(::pilcrow_web::axum::http::header::CONTENT_TYPE, \"text/html; x-ps-fragment=1\")], html).into_response()\n");
    }
    s.push_str("            } else {\n");
    if needs_req {
        s.push_str("            let mut __response = ::pilcrow_web::axum::response::Html(html).into_response();\n");
        s.push_str("            __resp_handle.apply_to(&mut __response);\n");
        s.push_str("            __response\n");
    } else {
        s.push_str("            ::pilcrow_web::axum::response::Html(html).into_response()\n");
    }
    s.push_str("            }\n");
    s
}

fn emit_render_binding(
    html_var: &str,
    render_expr: &str,
    error_mod: Option<&str>,
    has_resp_handle: bool,
    indent_levels: usize,
) -> String {
    let pad = "    ".repeat(indent_levels);
    let mut s = String::new();
    let _ = writeln!(s, "{pad}let {html_var} = match {render_expr} {{");
    let _ = writeln!(s, "{pad}    ::std::result::Result::Ok(html) => html,");
    let _ = writeln!(s, "{pad}    ::std::result::Result::Err(__err) => {{");
    let _ = writeln!(
        s,
        "{pad}        ::pilcrow_web::tracing::error!(error = %__err, \"template render failed\");"
    );
    if has_resp_handle {
        let _ = writeln!(s, "{pad}        let e = ::pilcrow_web::AppError::Internal;");
        s.push_str(&emit_app_error_body(error_mod, indent_levels + 2));
    } else {
        let _ = writeln!(
            s,
            "{pad}        return (::pilcrow_web::StatusCode::INTERNAL_SERVER_ERROR, \"template render failed\").into_response();"
        );
    }
    let _ = writeln!(s, "{pad}    }}");
    let _ = writeln!(s, "{pad}}};");
    s
}

fn route_config_error(
    entry: &GeneratedPageRoute,
    message: impl Into<String>,
    suggested_fix: impl Into<String>,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "invalid Pilcrow route configuration\nroute: {}\nmodule: {}\n\n{}\n\nsuggested fix: {}",
            entry.pattern,
            entry.symbol,
            message.into(),
            suggested_fix.into()
        ),
    )
}

/// Render the API `mod` tree file (`generated_api_mods.rs`).
///
/// Uses `#[path]` attributes pointing to the source directory so that
/// `include!()` from `OUT_DIR` resolves module files correctly.
pub fn render_generated_api_mods(
    api_entries: &[GeneratedApiRoute],
    src_root: &Path,
    hooks: HookFlags,
) -> String {
    let mut out = String::new();
    out.push_str("// @generated by pilcrow-routekit. Do not edit manually.\n");

    // Expose src/hooks.rs as `crate::hooks` so the generated shims can call into it.
    if hooks.has_handle || hooks.has_handle_error || hooks.has_init {
        let hooks_path = src_root.join("hooks.rs");
        let hooks_path_str = hooks_path.to_string_lossy().replace('\\', "/");
        let _ = writeln!(out, "#[path = \"{hooks_path_str}\"]");
        out.push_str("pub mod hooks;\n");
    }

    // Expose src/params/ as `crate::params` when the directory exists.
    let params_dir = src_root.join("params");
    if params_dir.exists()
        && let Ok(read_dir) = fs::read_dir(&params_dir)
    {
        let params_dir_str = params_dir.to_string_lossy().replace('\\', "/");
        let mut param_mods: Vec<String> = read_dir
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();
        param_mods.sort();
        if !param_mods.is_empty() {
            let _ = writeln!(out, "#[path = \"{params_dir_str}\"]");
            out.push_str("pub mod params {\n");
            for mod_name in &param_mods {
                let _ = writeln!(out, "    pub mod {mod_name};");
            }
            out.push_str("}\n");
        }
    }

    if api_entries.is_empty() {
        return out;
    }

    // Use #[path] to resolve mod files relative to the actual source tree
    let api_dir = src_root.join("api");
    let api_dir_str = api_dir.to_string_lossy().replace('\\', "/");
    let _ = writeln!(out, "#[path = \"{api_dir_str}\"]");
    out.push_str("pub mod api {\n");

    // Emit leaf modules only (strip the `api::` prefix)
    for entry in api_entries {
        let leaf = entry
            .module_path
            .strip_prefix("api::")
            .unwrap_or(&entry.module_path);
        // For nested modules like "users::id", we need nested mod declarations.
        // For simplicity, only handle single-level for now; nested will use #[path] too.
        if !leaf.contains("::") {
            let _ = writeln!(out, "    pub mod {leaf};");
        }
    }

    // Handle nested API modules
    let nested: Vec<_> = api_entries
        .iter()
        .filter_map(|e| {
            e.module_path
                .strip_prefix("api::")
                .filter(|l| l.contains("::"))
        })
        .collect();

    if !nested.is_empty() {
        // Build sub-tree for nested entries
        #[derive(Default)]
        struct SubNode {
            children: std::collections::BTreeMap<String, SubNode>,
            is_leaf: bool,
        }

        fn sub_insert(node: &mut SubNode, segments: &[&str]) {
            if segments.is_empty() {
                return;
            }
            let child = node.children.entry(segments[0].to_string()).or_default();
            if segments.len() == 1 {
                child.is_leaf = true;
            } else {
                sub_insert(child, &segments[1..]);
            }
        }

        fn sub_emit(node: &SubNode, indent: usize, base_path: &str) -> String {
            let mut out = String::new();
            let pad = "    ".repeat(indent);
            for (name, child) in &node.children {
                let child_path = format!("{base_path}/{name}");
                if child.is_leaf && child.children.is_empty() {
                    let _ = writeln!(out, "{pad}pub mod {name};");
                } else {
                    let _ = writeln!(out, "{pad}#[path = \"{child_path}\"]");
                    let _ = writeln!(out, "{pad}pub mod {name} {{");
                    out.push_str(&sub_emit(child, indent + 1, &child_path));
                    let _ = writeln!(out, "{pad}}}");
                }
            }
            out
        }

        let mut root = SubNode::default();
        for path in &nested {
            let segments: Vec<&str> = path.split("::").collect();
            sub_insert(&mut root, &segments);
        }
        out.push_str(&sub_emit(&root, 1, &api_dir_str));
    }

    out.push_str("}\n");
    out
}

/// Render the full app module that auto-wires all page and API routes.
///
/// This generates `generated_app.rs` containing:
/// - An include of `generated_templates.rs`
/// - A `build_router()` function returning a fully-wired `axum::Router`
///
/// API routes reference modules via `crate::` paths since the mod tree
/// is included separately at crate root level.
///
/// `error_module_for_page` maps each page symbol to its nearest `_error.html` module name.
/// `not_found_module` is the module name of `_not_found.html` (registered as axum fallback).
pub fn render_generated_app_module(
    page_entries: &[GeneratedPageRoute],
    api_entries: &[GeneratedApiRoute],
    maps: &AppCodegenMaps<'_>,
    hooks: HookFlags,
    has_react_assets: bool,
    has_solid_assets: bool,
) -> io::Result<String> {
    let AppCodegenMaps {
        load_map,
        layout_fields_map,
        error_module_for_page,
        not_found_module,
        loading_module_for_page,
        action_map,
        page_options_map,
        live_fields_map,
        has_live_fn_map,
        fsr_live_source_map,
        fsr_live_fields_map: _fsr_live_fields_map,
        fsr_default_revalidate_symbols,
        layout_chain_ids_map,
        page_slot_map,
    } = maps;
    let mut out = String::new();
    out.push_str("// @generated by pilcrow-routekit. Do not edit manually.\n\n");

    // Include generated templates
    out.push_str("#[allow(dead_code)]\n");
    out.push_str("mod __pilcrow_gen {\n");
    out.push_str("    include!(concat!(env!(\"OUT_DIR\"), \"/generated_templates.rs\"));\n");
    out.push_str("}\n\n");
    out.push_str("#[allow(dead_code)]\n");
    out.push_str("mod __pilcrow_react_assets {\n");
    out.push_str("    include!(concat!(env!(\"OUT_DIR\"), \"/generated_react_assets.rs\"));\n");
    out.push_str("}\n\n");
    out.push_str("#[allow(dead_code)]\n");
    out.push_str("mod __pilcrow_solid_assets {\n");
    out.push_str("    include!(concat!(env!(\"OUT_DIR\"), \"/generated_solid_assets.rs\"));\n");
    out.push_str("}\n\n");
    out.push_str("#[allow(dead_code)]\n");
    out.push_str("mod __pilcrow_react_ssr {\n");
    out.push_str("    include!(concat!(env!(\"OUT_DIR\"), \"/generated_react_ssr.rs\"));\n");
    out.push_str("}\n\n");
    // Exposes SSR bundle sources to pilcrow_start() so it can spawn the Node worker.
    out.push_str("#[allow(dead_code)]\n");
    out.push_str("pub fn __pilcrow_ssr_bundles() -> &'static [(&'static str, &'static str)] {\n");
    out.push_str("    __pilcrow_react_ssr::SSR_BUNDLES\n");
    out.push_str("}\n\n");

    if has_react_assets || has_solid_assets {
        out.push_str("#[allow(dead_code)]\n");
        out.push_str("async fn __pilcrow_serve_client_asset(\n");
        out.push_str("    ::pilcrow_web::axum::extract::Path(path): ::pilcrow_web::axum::extract::Path<String>,\n");
        out.push_str(") -> ::pilcrow_web::Response {\n");
        out.push_str("    use ::pilcrow_web::axum::response::IntoResponse as _;\n");
        if has_react_assets {
            out.push_str(
                "    if let Some((content_type, bytes)) = __pilcrow_react_assets::asset(&path) {\n",
            );
            out.push_str("        return (\n");
            out.push_str("            ::pilcrow_web::StatusCode::OK,\n");
            out.push_str("            [\n");
            out.push_str(
                "                (::pilcrow_web::axum::http::header::CONTENT_TYPE, content_type),\n",
            );
            out.push_str("                (::pilcrow_web::axum::http::header::CACHE_CONTROL, \"public, max-age=31536000, immutable\"),\n");
            out.push_str("            ],\n");
            out.push_str("            bytes,\n");
            out.push_str("        ).into_response();\n");
            out.push_str("    }\n");
        }
        if has_solid_assets {
            out.push_str(
                "    if let Some((content_type, bytes)) = __pilcrow_solid_assets::asset(&path) {\n",
            );
            out.push_str("        return (\n");
            out.push_str("            ::pilcrow_web::StatusCode::OK,\n");
            out.push_str("            [\n");
            out.push_str(
                "                (::pilcrow_web::axum::http::header::CONTENT_TYPE, content_type),\n",
            );
            out.push_str("                (::pilcrow_web::axum::http::header::CACHE_CONTROL, \"public, max-age=31536000, immutable\"),\n");
            out.push_str("            ],\n");
            out.push_str("            bytes,\n");
            out.push_str("        ).into_response();\n");
            out.push_str("    }\n");
        }
        out.push_str("    ::pilcrow_web::StatusCode::NOT_FOUND.into_response()\n");
        out.push_str("}\n\n");
    }

    // build_router function
    out.push_str("#[allow(dead_code)]\n");
    out.push_str("pub fn build_router() -> ::pilcrow_web::axum::Router {\n");
    out.push_str("    __pilcrow_register_codegen_revalidation();\n");
    out.push_str("    ::pilcrow_web::axum::Router::new()\n");
    if has_react_assets || has_solid_assets {
        out.push_str("        .route(\"/_pilcrow/client/*path\", ::pilcrow_web::axum::routing::get(__pilcrow_serve_client_asset))\n");
    }

    // Page routes
    for entry in page_entries {
        let page_load = load_map.get(&entry.symbol).copied().flatten();
        let chain_info = layout_fields_map.get(&entry.symbol);
        let error_mod = error_module_for_page.get(&entry.symbol).map(String::as_str);
        let loading_mod = loading_module_for_page
            .get(&entry.symbol)
            .map(String::as_str);
        let page_actions: Option<&Vec<ActionFn>> = action_map.get(&entry.symbol);

        let pattern = rust_string(&entry.pattern);
        let mod_name = &entry.symbol;
        let render_fn = &entry.render_symbol;
        let live_fields: &[String] = live_fields_map
            .get(&entry.symbol)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let _has_live_fn_for_page = *has_live_fn_map.get(&entry.symbol).unwrap_or(&false);
        // True when this route has inline Live (FSR) — use FSR client script instead of old live SSE.
        let has_fsr = fsr_live_source_map.contains_key(&entry.symbol);

        // Layout-aware navigation: layout chain IDs and page slot pattern.
        let ps_layout_chain: &[String] = layout_chain_ids_map
            .get(&entry.symbol)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let ps_page_slot: Option<&str> = page_slot_map.get(&entry.symbol).map(String::as_str);

        // Collect the active chain: (var_index, layout_mod, field_names, sig) for each layout
        // in the chain that has a load() function.
        let active_chain: Vec<(usize, &str, &Vec<String>, LoadSignature)> = chain_info
            .map(|info| {
                info.chain
                    .iter()
                    .enumerate()
                    .filter_map(|(i, (lmod, fields))| {
                        load_map
                            .get(lmod.as_str())
                            .copied()
                            .flatten()
                            .map(|sig| (i, lmod.as_str(), fields, sig))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let any_layout_load = !active_chain.is_empty();
        let any_layout_client = active_chain.iter().any(|(_, _, _, sig)| sig.wants_client);
        let page_wants_client = page_load.is_some_and(|s| s.wants_client);
        let needs_client = any_layout_client || page_wants_client;

        let any_layout_req = active_chain.iter().any(|(_, _, _, sig)| sig.consumes_req());
        let page_wants_req = page_load.is_some_and(|s| s.consumes_req());
        let has_param_matchers = !entry.param_matchers.is_empty();
        let has_typed_param_guards = entry
            .route_params
            .iter()
            .any(|param| matches!(param.rust_type.as_str(), "i64" | "u64"));
        let any_load_returns_result = page_load.is_some_and(|s| s.returns_result)
            || active_chain.iter().any(|(_, _, _, sig)| sig.returns_result);
        let needs_req = any_layout_req
            || page_wants_req
            || has_param_matchers
            || has_typed_param_guards
            || error_mod.is_some()
            || any_load_returns_result
            || !ps_layout_chain.is_empty();
        let page_wants_live = page_load.is_some_and(|s| s.wants_live);

        // PilcrowClient and Live are both FromRequestParts and must come before Req (FromRequest).
        // Order: client?, live?, req?
        let live_closure_arg = if page_wants_live {
            format!("__live: __pilcrow_gen::{mod_name}::Live")
        } else {
            String::new()
        };
        let closure_args = {
            let mut parts: Vec<&str> = Vec::new();
            if needs_client {
                parts.push("client: ::pilcrow_web::PilcrowClient");
            }
            // Live extractor is named __live to avoid shadowing the user's `live` local.
            // The generated load() call passes `__live` as the `live` argument.
            if page_wants_live {
                parts.push(&live_closure_arg);
            }
            if needs_req {
                parts.push("req: ::pilcrow_web::Req");
            }
            parts.join(", ")
        };

        let _fsr_json = page_options_map
            .get(&entry.symbol)
            .is_some_and(|o| o.fsr.json);

        let pattern_str = entry.pattern.as_str();
        let _ = writeln!(
            out,
            "        .route({pattern}, ::pilcrow_web::axum::routing::get(|{closure_args}| {{"
        );
        let _ = writeln!(
            out,
            "            use ::pilcrow_web::tracing::Instrument as _;"
        );
        let _ = writeln!(
            out,
            "            let __req_span = ::pilcrow_web::tracing::info_span!(\"GET\", http.route = {});",
            rust_string(pattern_str)
        );
        out.push_str("            async move {\n");

        // ── Param matcher guards ────────────────────────────────────────────────
        if has_param_matchers || has_typed_param_guards {
            out.push_str("            use ::pilcrow_web::axum::response::IntoResponse;\n");
            for param in entry
                .route_params
                .iter()
                .filter(|param| matches!(param.rust_type.as_str(), "i64" | "u64"))
            {
                let name = &param.name;
                let ty = &param.rust_type;
                let _ = writeln!(
                    out,
                    "            if req.params.get(\"{name}\").is_some_and(|__v| __v.parse::<{ty}>().is_err()) {{"
                );
                out.push_str("                return (::pilcrow_web::StatusCode::NOT_FOUND, \"not found\").into_response();\n");
                out.push_str("            }\n");
            }
            let mut sorted_matchers: Vec<(&String, &String)> =
                entry.param_matchers.iter().collect();
            sorted_matchers.sort_by_key(|(k, _)| k.as_str());
            for (param_name, matcher_mod) in &sorted_matchers {
                let _ = writeln!(
                    out,
                    "            if !crate::params::{matcher_mod}::match_param(req.params.get(\"{param_name}\").map(|s| s.as_str()).unwrap_or(\"\")) {{"
                );
                out.push_str("                return (::pilcrow_web::StatusCode::NOT_FOUND, \"not found\").into_response();\n");
                out.push_str("            }\n");
            }
        }

        if !any_layout_load && page_load.is_none() {
            // ── Case 1: static page ─────────────────────────────────────────────
            out.push_str("            use ::pilcrow_web::axum::response::IntoResponse;\n");
            if needs_req {
                out.push_str("            let __resp_handle = req.res.clone();\n");
            }
            let _ = writeln!(
                out,
                "            let props = __pilcrow_gen::{mod_name}::Props::default();"
            );
            out.push_str(&emit_render_binding(
                "html",
                &format!("__pilcrow_gen::{mod_name}::{render_fn}(props)"),
                error_mod,
                needs_req,
                3,
            ));
            out.push_str(&emit_loading_append(loading_mod, "html"));
            if !ps_layout_chain.is_empty() {
                if let Some(slot) = ps_page_slot {
                    out.push_str(&emit_ps_fragment_check(ps_layout_chain, slot));
                    out.push_str(&emit_ps_response(needs_req));
                } else if needs_req {
                    out.push_str("            let mut __response = ::pilcrow_web::axum::response::Html(html).into_response();\n");
                    out.push_str("            __resp_handle.apply_to(&mut __response);\n");
                    out.push_str("            __response\n");
                } else {
                    out.push_str(
                        "            ::pilcrow_web::axum::response::Html(html).into_response()\n",
                    );
                }
            } else if needs_req {
                out.push_str("            let mut __response = ::pilcrow_web::axum::response::Html(html).into_response();\n");
                out.push_str("            __resp_handle.apply_to(&mut __response);\n");
                out.push_str("            __response\n");
            } else {
                out.push_str(
                    "            ::pilcrow_web::axum::response::Html(html).into_response()\n",
                );
            }
        } else {
            out.push_str("            use ::pilcrow_web::axum::response::IntoResponse;\n");
            // Clone the response handle before req consumption so load() calls
            // can write to it and we can apply it after rendering.
            if needs_req {
                out.push_str("            let __resp_handle = req.res.clone();\n");
            }
            // ── Layout loads (one per active chain entry, outermost first) ───────
            // When a client is cloned: it must be cloned for every call except the
            // very last consumer (page or last layout).
            let _last_client_consumer_is_page = page_wants_client;
            let layout_client_consumers = active_chain
                .iter()
                .filter(|(_, _, _, s)| s.wants_client)
                .count();
            let mut client_clones_left = if needs_client {
                let total = layout_client_consumers + if page_wants_client { 1 } else { 0 };
                total.saturating_sub(1)
            } else {
                0
            };

            let layout_req_consumers = active_chain
                .iter()
                .filter(|(_, _, _, s)| s.consumes_req())
                .count();
            let ps_req_consumers =
                usize::from(!ps_layout_chain.is_empty() && ps_page_slot.is_some());
            let mut req_clones_left = if needs_req {
                let total =
                    layout_req_consumers + if page_wants_req { 1 } else { 0 } + ps_req_consumers;
                total.saturating_sub(1)
            } else {
                0
            };

            for (idx, layout_mod, _, lsig) in &active_chain {
                let req_arg = if lsig.wants_req {
                    if req_clones_left > 0 {
                        req_clones_left -= 1;
                        "req.clone()"
                    } else {
                        "req"
                    }
                } else {
                    ""
                };
                let client_arg = if lsig.wants_client {
                    if client_clones_left > 0 {
                        client_clones_left -= 1;
                        "client.clone()"
                    } else {
                        "client"
                    }
                } else {
                    ""
                };
                let layout_call_args = match (req_arg, client_arg) {
                    ("", "") => String::new(),
                    (r, "") => r.to_string(),
                    ("", cl) => cl.to_string(),
                    (r, cl) => format!("{r}, {cl}"),
                };
                let call_expr = format!("__pilcrow_gen::{layout_mod}::load({layout_call_args})");
                let awaited = if lsig.is_async {
                    format!("{call_expr}.await")
                } else {
                    call_expr
                };
                let var = format!("layout_data_{idx}");
                if lsig.returns_result {
                    let _ = writeln!(out, "            let {var} = match {awaited} {{");
                    out.push_str("                Ok(p) => p,\n");
                    out.push_str(&emit_error_branch(error_mod));
                    out.push_str("            };\n");
                } else {
                    let _ = writeln!(out, "            let {var} = {awaited};");
                }
            }

            // ── Page load ────────────────────────────────────────────────────────
            if let Some(psig) = page_load {
                let req_arg = if psig.consumes_req() {
                    let raw = if req_clones_left > 0 {
                        req_clones_left -= 1;
                        "req.clone()"
                    } else {
                        "req"
                    };
                    if psig.wants_page {
                        format!("::pilcrow_web::Page::from_req({raw})")
                    } else {
                        raw.to_string()
                    }
                } else {
                    String::new()
                };
                let client_arg = if psig.wants_client {
                    if client_clones_left > 0 {
                        client_clones_left -= 1;
                        "client.clone()"
                    } else {
                        "client"
                    }
                } else {
                    ""
                };
                // When load() declares a `live: Live` parameter, it was already extracted
                // as a closure argument via FromRequestParts (see closure_args below).
                let live_arg = if psig.wants_live { "__live" } else { "" };
                let page_call_args = match (req_arg.as_str(), client_arg, live_arg) {
                    ("", "", "") => String::new(),
                    (r, "", "") => r.to_string(),
                    ("", cl, "") => cl.to_string(),
                    ("", "", lv) => lv.to_string(),
                    (r, cl, "") => format!("{r}, {cl}"),
                    (r, "", lv) => format!("{r}, {lv}"),
                    ("", cl, lv) => format!("{cl}, {lv}"),
                    (r, cl, lv) => format!("{r}, {cl}, {lv}"),
                };
                let call_expr = format!("__pilcrow_gen::{mod_name}::load({page_call_args})");
                let awaited = if psig.is_async {
                    format!("{call_expr}.await")
                } else {
                    call_expr
                };
                if psig.returns_result {
                    let _ = writeln!(out, "            let page_data = match {awaited} {{");
                    out.push_str("                Ok(p) => p,\n");
                    out.push_str(&emit_error_branch(error_mod));
                    out.push_str("            };\n");
                } else {
                    let _ = writeln!(out, "            let page_data = {awaited};");
                }
            }

            // ── Construct props ──────────────────────────────────────────────────
            if any_layout_load {
                // Layout chain contributed fields → use __MergedProps.
                let info = chain_info.expect("chain_info present when active_chain is non-empty");
                let _ = writeln!(
                    out,
                    "            let props = __pilcrow_gen::{mod_name}::__MergedProps {{"
                );
                // Layout fields: each entry maps to its var (layout_data_{i}).
                for (idx, _, field_names, _) in &active_chain {
                    let var = format!("layout_data_{idx}");
                    for field in *field_names {
                        let _ = writeln!(out, "                {field}: {var}.{field},");
                    }
                }
                // Page fields: from page_data if page has load(), else Default.
                if page_load.is_some() {
                    for field in &info.page_field_names {
                        let _ = writeln!(out, "                {field}: page_data.{field},");
                    }
                } else {
                    for field in &info.page_field_names {
                        let _ = writeln!(out, "                {field}: Default::default(),");
                    }
                }
                out.push_str("            };\n");
            } else {
                // Only page has load() → Props directly.
                let _ = writeln!(out, "            let props = page_data;");
            }

            {
                out.push_str(&emit_render_binding(
                    "html",
                    &format!("__pilcrow_gen::{mod_name}::{render_fn}(props)"),
                    error_mod,
                    needs_req,
                    3,
                ));
                // ── FSR: inject client script before </head> ─────────────────
                if has_fsr {
                    let fsr_script = FSR_PATCH_SCRIPT;
                    let fsr_script_tag = format!("<script>{fsr_script}</script>");
                    let fsr_script_lit = rust_string(&fsr_script_tag);
                    let _ = writeln!(
                        out,
                        "            const __FSR_SCRIPT: &str = {fsr_script_lit};"
                    );
                    out.push_str(
                        "            let html = if let Some(__pos) = html.find(\"</head>\") {\n",
                    );
                    out.push_str("                let mut __s = String::with_capacity(html.len() + __FSR_SCRIPT.len());\n");
                    out.push_str("                __s.push_str(&html[..__pos]);\n");
                    out.push_str("                __s.push_str(__FSR_SCRIPT);\n");
                    out.push_str("                __s.push_str(&html[__pos..]);\n");
                    out.push_str("                __s\n");
                    out.push_str("            } else {\n");
                    out.push_str("                format!(\"{}{}\", __FSR_SCRIPT, html)\n");
                    out.push_str("            };\n");
                }
                out.push_str(&emit_loading_append(loading_mod, "html"));
                if !ps_layout_chain.is_empty() {
                    if let Some(slot) = ps_page_slot {
                        out.push_str(&emit_ps_fragment_check(ps_layout_chain, slot));
                        out.push_str(&emit_ps_response(true)); // live props branch always has needs_req=true
                    } else {
                        out.push_str("            let mut __response = ::pilcrow_web::axum::response::Html(html).into_response();\n");
                        out.push_str("            __resp_handle.apply_to(&mut __response);\n");
                        out.push_str("            __response\n");
                    }
                } else if needs_req {
                    out.push_str("            let mut __response = ::pilcrow_web::axum::response::Html(html).into_response();\n");
                    out.push_str("            __resp_handle.apply_to(&mut __response);\n");
                    out.push_str("            __response\n");
                } else {
                    out.push_str(
                        "            ::pilcrow_web::axum::response::Html(html).into_response()\n",
                    );
                }
            }
        }

        out.push_str("            }.instrument(__req_span)\n        }))\n");

        // ── Action POST route (same URL, dispatched by `?/<name>`) ──────────────
        if let Some(actions) = page_actions
            && !actions.is_empty()
        {
            out.push_str(&emit_action_route(
                actions,
                &entry.pattern,
                mod_name,
                error_mod,
            ));
        }

        // ── Trailing slash redirect routes ───────────────────────────────────────
        if let Some(opts) = page_options_map.get(&entry.symbol) {
            let base = &entry.pattern;
            match opts.trailing_slash {
                TrailingSlash::Always if !base.ends_with('/') && base != "/" => {
                    let slashed = rust_string(&format!("{base}/"));
                    let bare = rust_string(base.as_str());
                    let _ = writeln!(
                        out,
                        "        .route({bare}, ::pilcrow_web::axum::routing::get(|| async move {{"
                    );
                    let _ = writeln!(
                        out,
                        "            ::pilcrow_web::axum::response::Redirect::permanent({slashed})"
                    );
                    out.push_str("        }))\n");
                }
                TrailingSlash::Ignore if !base.ends_with('/') && base != "/" => {
                    let slashed = rust_string(&format!("{base}/"));
                    let bare = rust_string(base.as_str());
                    let _ = writeln!(
                        out,
                        "        .route({slashed}, ::pilcrow_web::axum::routing::get(|| async move {{"
                    );
                    let _ = writeln!(
                        out,
                        "            ::pilcrow_web::axum::response::Redirect::permanent({bare})"
                    );
                    out.push_str("        }))\n");
                }
                _ => {}
            }
        }
    }

    // API routes — reference via crate:: since mod tree is at crate root
    for entry in api_entries {
        let pattern = rust_string(&entry.pattern);
        let mod_path = &entry.module_path;
        let _ = writeln!(out, "        .nest({pattern}, crate::{mod_path}::router())");
    }

    // Not-found fallback — registered last so it matches any unhandled request.
    if let Some(nf_mod) = not_found_module {
        let render_fn = format!("render_{nf_mod}");
        out.push_str("        .fallback(|| async {\n");
        let _ = writeln!(
            out,
            "            let props = __pilcrow_gen::{nf_mod}::Props {{}};"
        );
        let _ = writeln!(
            out,
            "            let html = __pilcrow_gen::{nf_mod}::{render_fn}(props)"
        );
        out.push_str(
            "                .unwrap_or_else(|_| \"<h1>404 Not Found</h1>\".to_string());\n",
        );
        out.push_str("            (\n");
        out.push_str("                ::pilcrow_web::StatusCode::NOT_FOUND,\n");
        out.push_str("                ::pilcrow_web::axum::response::Html(html)\n");
        out.push_str("            )\n");
        out.push_str("        })\n");
    }

    // Layer order (innermost to outermost — last .layer() call is outermost):
    //
    //   routes  ←  handle_error  ←  handle  ←  CSRF
    //
    // `handle_error` wraps routes directly so it sees every 5xx response.
    // `handle` wraps both, so locals it sets are visible to handle_error and routes.
    // CSRF is outermost: it rejects bad cross-origin mutations before any user code runs.
    if hooks.has_handle_error {
        out.push_str(
            "        .layer(::pilcrow_web::axum::middleware::from_fn(__pilcrow_error_handler))\n",
        );
    }
    if hooks.has_handle {
        out.push_str(
            "        .layer(::pilcrow_web::axum::middleware::from_fn(__pilcrow_handle))\n",
        );
    }

    // CSRF is always-on and outermost: state-changing form submissions with a
    // cross-origin Origin/Referer are rejected before any user code runs.
    out.push_str("        .layer(::pilcrow_web::axum::middleware::from_fn(::pilcrow_web::__csrf_middleware))\n");

    out.push_str("}\n");

    // ── codegen revalidation registration ────────────────────────────────────
    out.push('\n');
    out.push_str("fn __pilcrow_register_codegen_revalidation() {\n");
    out.push_str("    static __PILCROW_REVALIDATION_REGISTERED: ::std::sync::OnceLock<()> = ::std::sync::OnceLock::new();\n");
    out.push_str("    let _ = __PILCROW_REVALIDATION_REGISTERED.get_or_init(|| {\n");
    // Collect scheduled invalidations from per-field #[revalidate(N)] attrs.
    // Same route + same interval → one shared timer (dedup by (module, interval)).
    // Synthetic dep key: `{module}::__revalidate_{N}s`
    use std::collections::BTreeMap;
    let mut deduped: BTreeMap<(String, u64), ()> = BTreeMap::new();
    for entry in page_entries {
        if let Some(opts) = page_options_map.get(&entry.symbol) {
            for attr in opts.fsr.live_field_attrs.values() {
                if let Some(secs) = attr.revalidate_secs {
                    deduped.insert((entry.symbol.clone(), secs), ());
                }
            }
        }
    }
    let scheduled: Vec<(String, u64)> = deduped
        .into_keys()
        .map(|(module, secs)| (format!("{module}::__revalidate_{secs}s"), secs))
        .collect();
    if !scheduled.is_empty() {
        out.push_str(
            "        ::pilcrow_web::__register_codegen_scheduled_invalidations(::std::vec![\n",
        );
        for (dep_key, secs) in &scheduled {
            let key_lit = rust_string(dep_key);
            let _ = writeln!(
                out,
                "            ::pilcrow_web::ScheduledInvalidation::new({key_lit}, ::std::time::Duration::from_secs({secs}u64)),"
            );
        }
        out.push_str("        ]);\n");
    }
    // Register routes that need the global/default revalidation timer.
    let mut default_routes: Vec<&str> = fsr_default_revalidate_symbols
        .iter()
        .map(String::as_str)
        .collect();
    default_routes.sort();
    if !default_routes.is_empty() {
        out.push_str(
            "        ::pilcrow_web::__register_codegen_default_revalidate_routes(::std::vec![\n",
        );
        for route in &default_routes {
            let route_lit = rust_string(route);
            let _ = writeln!(out, "            {route_lit}.to_string(),");
        }
        out.push_str("        ]);\n");
    }
    out.push_str("    });\n");
    out.push_str("}\n");

    // ── __pilcrow_init: called before the server starts accepting connections ─
    out.push('\n');
    out.push_str("pub async fn __pilcrow_init() {\n");
    out.push_str("    __pilcrow_register_codegen_revalidation();\n");
    if hooks.has_init {
        out.push_str("    crate::hooks::init().await;\n");
    }
    out.push_str("}\n");

    // ── handle shim: extracts Req (body stays intact), calls hooks::handle ────
    if hooks.has_handle {
        out.push('\n');
        out.push_str("async fn __pilcrow_handle(\n");
        out.push_str("    req: ::pilcrow_web::axum::extract::Request,\n");
        out.push_str("    next: ::pilcrow_web::axum::middleware::Next,\n");
        out.push_str(") -> ::pilcrow_web::axum::response::Response {\n");
        out.push_str("    let (mut parts, body) = req.into_parts();\n");
        out.push_str(
            "    let pilcrow_req = ::pilcrow_web::Req::__from_middleware_parts(&mut parts, &()).await;\n",
        );
        out.push_str(
            "    let forwarded = ::pilcrow_web::axum::extract::Request::from_parts(parts, body);\n",
        );
        out.push_str("    let pilcrow_next = ::pilcrow_web::Next::new(next, forwarded);\n");
        out.push_str("    crate::hooks::handle(pilcrow_req, pilcrow_next).await\n");
        out.push_str("}\n");
    }

    // ── handle_error shim: snapshots Req, intercepts 5xx responses ───────────
    if hooks.has_handle_error {
        out.push('\n');
        out.push_str("async fn __pilcrow_error_handler(\n");
        out.push_str("    req: ::pilcrow_web::axum::extract::Request,\n");
        out.push_str("    next: ::pilcrow_web::axum::middleware::Next,\n");
        out.push_str(") -> ::pilcrow_web::axum::response::Response {\n");
        out.push_str("    let (parts, body) = req.into_parts();\n");
        out.push_str("    let err_req = ::pilcrow_web::Req::__from_error_parts(&parts);\n");
        out.push_str(
            "    let forwarded = ::pilcrow_web::axum::extract::Request::from_parts(parts, body);\n",
        );
        out.push_str("    let response = next.run(forwarded).await;\n");
        out.push_str("    if response.status().is_server_error() {\n");
        out.push_str("        let hook_error = ::pilcrow_web::HookError {\n");
        out.push_str("            status: response.status().as_u16(),\n");
        out.push_str(
            "            message: response.status().canonical_reason().unwrap_or(\"Internal Server Error\").to_string(),\n",
        );
        out.push_str("            source: None,\n");
        out.push_str("        };\n");
        out.push_str(
            "        if let Some(r) = crate::hooks::handle_error(&hook_error, &err_req).await {\n",
        );
        out.push_str("            return r;\n");
        out.push_str("        }\n");
        out.push_str("    }\n");
        out.push_str("    response\n");
        out.push_str("}\n");
    }

    Ok(out)
}

/// Generate and write the app module and API mods files.
#[allow(clippy::too_many_arguments)]
pub fn write_generated_app_module(
    page_entries: &[GeneratedPageRoute],
    api_entries: &[GeneratedApiRoute],
    maps: &AppCodegenMaps<'_>,
    hooks: HookFlags,
    has_react_assets: bool,
    has_solid_assets: bool,
    src_root: &Path,
    out_dir: impl AsRef<Path>,
) -> io::Result<()> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;

    let app_source = render_generated_app_module(
        page_entries,
        api_entries,
        maps,
        hooks,
        has_react_assets,
        has_solid_assets,
    )?;
    fs::write(out_dir.join("generated_app.rs"), app_source)?;

    let mods_source = render_generated_api_mods(api_entries, src_root, hooks);
    fs::write(out_dir.join("generated_api_mods.rs"), mods_source)?;

    Ok(())
}

/// Returns the FSR inline patch script. Exposed for tests only.
#[cfg(test)]
pub fn fsr_patch_script() -> &'static str {
    FSR_PATCH_SCRIPT
}
