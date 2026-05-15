use super::*;

/// Emit the body that runs when an `AppError` (bound to `e`) needs to be
/// converted into a response. Used from both `Err(e) => { ... }` match arms
/// and from the unknown-action branch of the dispatch table.
///
/// `AppError::Redirect` is handled first — it issues a 303 before any error-page logic.
/// If `error_mod` is `Some`, other errors render the scoped `_error.html` page.
/// Otherwise they fall back to a plain-text 500.
///
/// `req.res` modifiers (toasts, headers, cookies) are applied to both the
/// redirect response and the error-page response so middleware / load-phase
/// side-effects are not silently dropped.
///
/// `indent_levels` sets the number of 4-space indents on the first line; every
/// subsequent line is indented relative to that. This lets callers drop the
/// body into arbitrarily-nested `match` arms.
pub fn emit_app_error_body(error_mod: Option<&str>, indent_levels: usize) -> String {
    let pad = "    ".repeat(indent_levels);
    let mut s = String::new();

    // Redirect short-circuit
    let _ = writeln!(
        s,
        "{pad}if let ::pilcrow_web::AppError::Redirect(ref __path) = e {{"
    );
    let _ = writeln!(
        s,
        "{pad}    let mut __redir = ::pilcrow_web::axum::response::Redirect::to(__path).into_response();"
    );
    let _ = writeln!(s, "{pad}    __resp_handle.apply_to(&mut __redir);");
    let _ = writeln!(s, "{pad}    return __redir;");
    let _ = writeln!(s, "{pad}}}");

    if let Some(err_mod) = error_mod {
        let render_fn = format!("render_{err_mod}");
        let _ = writeln!(s, "{pad}let __status = e.status_code();");
        let _ = writeln!(
            s,
            "{pad}let __err_props = __pilcrow_gen::{err_mod}::Props {{"
        );
        let _ = writeln!(s, "{pad}    status: __status,");
        let _ = writeln!(s, "{pad}    message: e.to_string(),");
        let _ = writeln!(s, "{pad}}};");
        let _ = writeln!(
            s,
            "{pad}let __err_html = __pilcrow_gen::{err_mod}::{render_fn}(__err_props)"
        );
        let _ = writeln!(
            s,
            "{pad}    .unwrap_or_else(|_| format!(\"<h1>{{}} Error</h1>\", __status));"
        );
        let _ = writeln!(s, "{pad}let mut __err_resp = (");
        let _ = writeln!(s, "{pad}    ::pilcrow_web::StatusCode::from_u16(__status)");
        let _ = writeln!(
            s,
            "{pad}        .unwrap_or(::pilcrow_web::StatusCode::INTERNAL_SERVER_ERROR),"
        );
        let _ = writeln!(
            s,
            "{pad}    ::pilcrow_web::axum::response::Html(__err_html)"
        );
        let _ = writeln!(s, "{pad}).into_response();");
        let _ = writeln!(s, "{pad}__resp_handle.apply_to(&mut __err_resp);");
        let _ = writeln!(s, "{pad}return __err_resp;");
    } else {
        let _ = writeln!(
            s,
            "{pad}let mut __err_resp = (::pilcrow_web::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();"
        );
        let _ = writeln!(s, "{pad}__resp_handle.apply_to(&mut __err_resp);");
        let _ = writeln!(s, "{pad}return __err_resp;");
    }
    s
}

/// Generate the `Err(e) => { ... },` match arm at the load/action call site.
pub fn emit_error_branch(error_mod: Option<&str>) -> String {
    emit_error_branch_indented(error_mod, 4)
}

/// Same as [`emit_error_branch`] but lets the caller specify the indent level
/// of the `Err(e) =>` line (in 4-space increments). Used for the dispatch-
/// table inner match arms, which sit one level deeper than a top-level match.
pub fn emit_error_branch_indented(error_mod: Option<&str>, indent_levels: usize) -> String {
    let pad = "    ".repeat(indent_levels);
    let mut s = String::new();
    let _ = writeln!(s, "{pad}Err(e) => {{");
    s.push_str(&emit_app_error_body(error_mod, indent_levels + 1));
    let _ = writeln!(s, "{pad}}},");
    s
}

/// Emit the unknown-action fallback body (bound `e: AppError`, no match wrapper).
pub fn emit_app_error_branch_body(error_mod: Option<&str>, indent_levels: usize) -> String {
    emit_app_error_body(error_mod, indent_levels)
}

/// Append the loading skeleton `<template>` to the rendered page HTML.
///
/// If `loading_mod` is `Some`, emits code that appends
/// `<template id="__pilcrow_loading" hidden>…</template>` to `html_var`.
/// `html_var` is the name of the local binding that holds the rendered HTML
/// (typically `"html"` for simple routes and `"__shell_html"` for async-field routes).
/// If `None`, emits nothing.
pub fn emit_loading_append(loading_mod: Option<&str>, html_var: &str) -> String {
    if let Some(lmod) = loading_mod {
        let render_fn = format!("render_{lmod}");
        let mut s = String::new();
        let _ = writeln!(
            s,
            "            let __loading_html = __pilcrow_gen::{lmod}::{render_fn}(__pilcrow_gen::{lmod}::Props {{}}).unwrap_or_default();"
        );
        let _ = writeln!(
            s,
            "            let {html_var} = format!(\"{{{html_var}}}<template id=\\\"__pilcrow_loading\\\" hidden>{{__loading_html}}</template>\");"
        );
        s
    } else {
        String::new()
    }
}

/// Generate a single POST `.route(...)` that dispatches to one of the page's
/// discovered named action handlers.
///
/// The client POSTs to the page URL with `?/<name>` to invoke `fn <name>`.
/// An unknown action returns `404 Not Found`. The dispatch table is static —
/// handlers are discovered at build time by [`instrument_frontmatter`].
pub fn emit_action_route(
    actions: &[ActionFn],
    pattern: &str,
    mod_name: &str,
    error_mod: Option<&str>,
) -> String {
    let pattern_lit = rust_string(pattern);
    let mut s = String::new();
    let _ = writeln!(
        s,
        "        .route({pattern_lit}, ::pilcrow_web::axum::routing::post(|req: ::pilcrow_web::Req| async move {{"
    );
    s.push_str("            use ::pilcrow_web::axum::response::IntoResponse;\n");
    s.push_str("            use ::pilcrow_web::ResponseExt;\n");
    s.push_str("            let __resp_handle = req.res.clone();\n");
    s.push_str("            let __action = req.action().to_owned();\n");
    s.push_str("            let mut __response = match __action.as_str() {\n");

    for action in actions {
        let name = rust_string(&action.name);
        let fn_ident = &action.name;
        let call_arg = if action.wants_req { "req" } else { "" };
        let call_expr = format!("__pilcrow_gen::{mod_name}::{fn_ident}({call_arg})");
        let awaited = if action.is_async {
            format!("{call_expr}.await")
        } else {
            call_expr
        };

        let _ = writeln!(s, "                {name} => match {awaited} {{");
        s.push_str("                    Ok(r) => r,\n");
        s.push_str(&emit_error_branch_indented(error_mod, 5));
        s.push_str("                },\n");
    }

    // Unknown action → 404 with the same error-page rendering as other errors.
    s.push_str("                _ => {\n");
    s.push_str("                    let e = ::pilcrow_web::AppError::NotFound(format!(\"unknown action: {}\", __action));\n");
    s.push_str(&emit_app_error_branch_body(error_mod, 5));
    s.push_str("                }\n");
    s.push_str("            };\n");
    s.push_str("            __resp_handle.apply_to(&mut __response);\n");
    s.push_str("            __response\n");
    s.push_str("        }))\n");
    s
}
