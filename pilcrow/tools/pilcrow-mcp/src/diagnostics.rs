use crate::{
    validation::{validate_implementation, Finding, Severity},
    workspace::{parse_code_behind, resolve_project, scan_project},
};
use anyhow::Result;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct DiagnosticFinding {
    pub finding_id: String,
    pub severity: Severity,
    pub rule_id: String,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub source_ref: Option<String>,
    pub suggested_fix: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnoseResult {
    pub scope: String,
    pub findings: Vec<DiagnosticFinding>,
    pub summary: String,
    pub error_count: usize,
    pub warning_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixProposal {
    pub finding_id: String,
    pub rule_id: String,
    pub file: Option<String>,
    pub description: String,
    pub patch_hint: String,
    pub safe_to_auto_apply: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyFixResult {
    pub finding_id: String,
    pub applied: bool,
    pub dry_run: bool,
    pub message: String,
}

pub fn diagnose_project(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
) -> Result<DiagnoseResult> {
    let context = scan_project(current_root, project_root, manifest_path)?;
    let resolved = resolve_project(current_root, project_root, manifest_path)?;
    let _app_root = &resolved.app_root;

    let mut findings = Vec::new();
    let mut counter = 0usize;

    // Check for missing _not_found.html fallback
    if !context.has_not_found_fallback {
        findings.push(make_finding(
            &mut counter,
            Severity::Warning,
            "pilcrow-missing-not-found",
            "No pages/_not_found.html found. Pilcrow will return a generic 404 for unmatched routes.",
            None,
            None,
            "Create pages/_not_found.html with a user-friendly 404 page.",
        ));
    }

    // Check for routes with code-behind parse errors
    for node in &context.route_graph {
        if let Some(cb) = &node.code_behind {
            if let Some(err) = &cb.parse_error {
                findings.push(make_finding(
                    &mut counter,
                    Severity::Error,
                    "pilcrow-code-behind-parse-error",
                    &format!("Code-behind parse error in {}: {}", node.file_path, err),
                    Some(&format!("pages/{}", node.file_path.replace(".html", ".rs"))),
                    None,
                    "Fix the Rust syntax error in the code-behind file.",
                ));
            }
        }
    }

    // Check for routes with load() but wrong signatures
    for node in &context.route_graph {
        if let Some(cb) = &node.code_behind {
            if cb.has_load && !cb.load_is_async {
                findings.push(make_finding(
                    &mut counter,
                    Severity::Error,
                    "pilcrow-load-not-async",
                    &format!(
                        "load() in {} is not async. The build pipeline requires async load().",
                        node.file_path
                    ),
                    Some(&format!("pages/{}", node.file_path.replace(".html", ".rs"))),
                    None,
                    "Change `fn load` to `async fn load`.",
                ));
            }
        }
    }

    // Check for routes without any layout coverage
    let routes_without_any_layout: Vec<_> = context
        .route_graph
        .iter()
        .filter(|n| n.layout_chain.is_empty())
        .collect();
    if routes_without_any_layout.len() > 1 {
        findings.push(make_finding(
            &mut counter,
            Severity::Info,
            "pilcrow-no-shared-layout",
            &format!(
                "{} routes have no _layout.html in their hierarchy. Consider a root layout for shared chrome.",
                routes_without_any_layout.len()
            ),
            None,
            None,
            "Create pages/_layout.html with navigation, head, and body chrome shared by all pages.",
        ));
    }

    // Check for missing loading skeletons on routes with deferred fields
    for node in &context.route_graph {
        if let Some(cb) = &node.code_behind {
            if cb.has_deferred && !node.has_loading {
                findings.push(make_finding(
                    &mut counter,
                    Severity::Warning,
                    "pilcrow-deferred-no-loading-skeleton",
                    &format!(
                        "Route {} uses AsyncValue<T> but has no _loading.html in its hierarchy. Users may see a blank page during streaming.",
                        node.url_pattern
                    ),
                    Some(&format!("pages/{}", node.file_path)),
                    None,
                    "Add a _loading.html sibling or ancestor with skeleton content for this route.",
                ));
            }
        }
    }

    // Validate all Rust code-behind files
    for node in &context.route_graph {
        if node.has_code_behind {
            let rs_path = format!(
                "{}",
                resolved
                    .app_root
                    .join("pages")
                    .join(node.file_path.replace(".html", ".rs"))
                    .display()
            );
            if let Ok(source) = fs::read_to_string(&rs_path) {
                let rel = format!("pages/{}", node.file_path.replace(".html", ".rs"));
                let report = validate_implementation(&source, Some(&rel), Some("rust"));
                for vf in report.findings {
                    findings.push(validation_finding_to_diag(&mut counter, vf));
                }
            }
        }
    }

    // Check middleware exists if Req.locals is used anywhere
    if context.middleware.is_none() && context.routes.len() > 3 {
        findings.push(make_finding(
            &mut counter,
            Severity::Info,
            "pilcrow-no-middleware",
            "No hooks.rs found. If you need auth, tracing, or shared request locals across pages, add a middleware.",
            None,
            None,
            "Create hooks.rs with `pub async fn middleware(req: Req, next: Next) -> Response { ... }`.",
        ));
    }

    // Codegen status check
    if context.generated_out_dir.path.is_none() {
        findings.push(make_finding(
            &mut counter,
            Severity::Warning,
            "pilcrow-no-generated-artifacts",
            "No generated OUT_DIR found. The build pipeline has not run or the output was cleaned.",
            None,
            None,
            "Run `cargo build` in the web app directory or use codegen_build tool.",
        ));
    } else if !context.generated_out_dir.generated_app {
        findings.push(make_finding(
            &mut counter,
            Severity::Error,
            "pilcrow-missing-generated-app",
            "OUT_DIR exists but generated_app.rs is missing. The build may be stale or incomplete.",
            context.generated_out_dir.path.as_deref(),
            None,
            "Run `cargo build` to regenerate. Check for build errors with codegen_build.",
        ));
    }

    // Check for api routes with scan
    for api in &context.api_routes {
        let abs = resolved.app_root.join("api").join(&api.path);
        if let Ok(source) = fs::read_to_string(&abs) {
            if !source.contains("pub fn router(") {
                findings.push(make_finding(
                    &mut counter,
                    Severity::Error,
                    "pilcrow-api-missing-router",
                    &format!("API route {} does not export `pub fn router()`.", api.path),
                    Some(&format!("api/{}", api.path)),
                    None,
                    "Add `pub fn router() -> axum::Router { Router::new().route(...) }`.",
                ));
            }
        }
    }

    let error_count = findings
        .iter()
        .filter(|f| matches!(f.severity, Severity::Error))
        .count();
    let warning_count = findings
        .iter()
        .filter(|f| matches!(f.severity, Severity::Warning))
        .count();

    let summary = if findings.is_empty() {
        "No issues found.".to_string()
    } else {
        format!(
            "{} issue(s) found: {} error(s), {} warning(s), {} info(s).",
            findings.len(),
            error_count,
            warning_count,
            findings.len() - error_count - warning_count
        )
    };

    Ok(DiagnoseResult {
        scope: context.project_root,
        findings,
        summary,
        error_count,
        warning_count,
    })
}

pub fn diagnose_route(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
    route: &str,
) -> Result<DiagnoseResult> {
    let resolved = resolve_project(current_root, project_root, manifest_path)?;
    let pages = resolved.app_root.join("pages");

    let mut findings = Vec::new();
    let mut counter = 0usize;

    // Find the route file
    let route_normalized = route.trim_start_matches('/');
    let candidates = [
        format!("{}/index.html", route_normalized),
        format!("{}.html", route_normalized),
        if route_normalized.is_empty() {
            "index.html".to_string()
        } else {
            format!("{route_normalized}/index.html")
        },
    ];

    let mut found_html: Option<PathBuf> = None;
    for c in &candidates {
        let abs = pages.join(c);
        if abs.exists() {
            found_html = Some(abs);
            break;
        }
    }

    if found_html.is_none() {
        findings.push(make_finding(
            &mut counter,
            Severity::Error,
            "pilcrow-route-html-missing",
            &format!("No HTML template found for route '{}'. Expected at pages/{}/index.html or pages/{}.html.", route, route_normalized, route_normalized),
            None,
            None,
            "Create the missing .html template file.",
        ));
        return Ok(DiagnoseResult {
            scope: route.to_string(),
            findings,
            summary: "Route HTML template not found.".to_string(),
            error_count: 1,
            warning_count: 0,
        });
    }

    let html_abs = found_html.unwrap();
    let rs_abs = html_abs.with_extension("rs");

    // Check code-behind
    if rs_abs.exists() {
        if let Some(cb) = parse_code_behind(&rs_abs) {
            if let Some(err) = &cb.parse_error {
                findings.push(make_finding(
                    &mut counter,
                    Severity::Error,
                    "pilcrow-code-behind-parse-error",
                    &format!("Parse error: {err}"),
                    Some(&rs_abs.to_string_lossy()),
                    None,
                    "Fix the Rust syntax error.",
                ));
            }
            if cb.has_load && !cb.load_is_async {
                findings.push(make_finding(
                    &mut counter,
                    Severity::Error,
                    "pilcrow-load-not-async",
                    "load() is not async. Must be `pub async fn load`.",
                    Some(&rs_abs.to_string_lossy()),
                    None,
                    "Add `async` keyword to load().",
                ));
            }
            if cb.has_load && cb.has_props {
                // Check Props fields for AsyncValue/AsyncHtml without loading skeleton
                if cb.has_deferred {
                    findings.push(make_finding(
                        &mut counter,
                        Severity::Info,
                        "pilcrow-deferred-check-loading",
                        "This route uses AsyncValue<T> or AsyncHtml. Verify a _loading.html skeleton exists for this route's hierarchy.",
                        Some(&html_abs.to_string_lossy()),
                        None,
                        "Add _loading.html in this directory or a parent directory.",
                    ));
                }
            }
            if !cb.has_props && !cb.action_names.is_empty() {
                findings.push(make_finding(
                    &mut counter,
                    Severity::Warning,
                    "pilcrow-actions-without-props",
                    "Code-behind has action handlers but no Props struct. If this page needs a load function too, add Props.",
                    Some(&rs_abs.to_string_lossy()),
                    None,
                    "Add `pub struct Props { ... }` and a matching load() if the page needs server data.",
                ));
            }
        }

        // Run full validation
        if let Ok(source) = fs::read_to_string(&rs_abs) {
            let report =
                validate_implementation(&source, Some(&rs_abs.to_string_lossy()), Some("rust"));
            for vf in report.findings {
                findings.push(validation_finding_to_diag(&mut counter, vf));
            }
        }
    }

    // Check HTML template
    if let Ok(source) = fs::read_to_string(&html_abs) {
        let report =
            validate_implementation(&source, Some(&html_abs.to_string_lossy()), Some("html"));
        for vf in report.findings {
            findings.push(validation_finding_to_diag(&mut counter, vf));
        }
    }

    let error_count = findings
        .iter()
        .filter(|f| matches!(f.severity, Severity::Error))
        .count();
    let warning_count = findings
        .iter()
        .filter(|f| matches!(f.severity, Severity::Warning))
        .count();

    let summary = if findings.is_empty() {
        format!("Route '{route}' looks healthy.")
    } else {
        format!(
            "{} issue(s) in route '{}': {} error(s), {} warning(s).",
            findings.len(),
            route,
            error_count,
            warning_count
        )
    };

    Ok(DiagnoseResult {
        scope: route.to_string(),
        findings,
        summary,
        error_count,
        warning_count,
    })
}

pub fn diagnose_codegen(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
) -> Result<DiagnoseResult> {
    use crate::workspace::find_out_dir;

    let resolved = resolve_project(current_root, project_root, manifest_path)?;
    let mut findings = Vec::new();
    let mut counter = 0usize;

    let out_dir = match find_out_dir(&resolved.manifest_path) {
        Ok(d) => d,
        Err(e) => {
            findings.push(make_finding(
                &mut counter,
                Severity::Error,
                "pilcrow-no-out-dir",
                &format!("OUT_DIR not found: {e}"),
                None,
                None,
                "Run `cargo build` in the web app to generate routekit artifacts.",
            ));
            return Ok(DiagnoseResult {
                scope: "codegen".to_string(),
                findings,
                summary: "OUT_DIR not found — build has not run.".to_string(),
                error_count: 1,
                warning_count: 0,
            });
        }
    };

    let required_files = [
        "generated_app.rs",
        "generated_routes.rs",
        "generated_api_mods.rs",
    ];
    for req in &required_files {
        if !out_dir.join(req).exists() {
            findings.push(make_finding(
                &mut counter,
                Severity::Error,
                "pilcrow-missing-generated-file",
                &format!("Generated file {req} is missing from OUT_DIR."),
                Some(&out_dir.join(req).to_string_lossy()),
                None,
                &format!("Run `cargo build` to regenerate {req}. Check build output for errors."),
            ));
        }
    }

    // Cross-check: routes in pages should appear in generated_app.rs
    let pages = resolved.app_root.join("pages");
    if let Ok(generated_app) = fs::read_to_string(out_dir.join("generated_app.rs")) {
        let src_routes: Vec<_> = collect_html_routes(&pages);
        for route in &src_routes {
            let module_hint = route
                .trim_end_matches(".html")
                .replace(['/', '-', '.'], "_");
            if !generated_app.contains(&module_hint) {
                findings.push(make_finding(
                    &mut counter,
                    Severity::Warning,
                    "pilcrow-route-not-in-generated-app",
                    &format!("Route {} does not appear in generated_app.rs (looked for module hint '{module_hint}'). It may be stale.", route),
                    Some(&format!("pages/{route}")),
                    None,
                    "Run `cargo build` to regenerate. Check for syntax errors in the code-behind.",
                ));
            }
        }
    }

    let error_count = findings
        .iter()
        .filter(|f| matches!(f.severity, Severity::Error))
        .count();
    let warning_count = findings
        .iter()
        .filter(|f| matches!(f.severity, Severity::Warning))
        .count();

    let summary = if findings.is_empty() {
        "Codegen artifacts look healthy.".to_string()
    } else {
        format!(
            "{} issue(s) in codegen: {} error(s), {} warning(s).",
            findings.len(),
            error_count,
            warning_count
        )
    };

    Ok(DiagnoseResult {
        scope: out_dir.to_string_lossy().into_owned(),
        findings,
        summary,
        error_count,
        warning_count,
    })
}

pub fn propose_fix(findings: &[DiagnosticFinding], finding_id: &str) -> Option<FixProposal> {
    let finding = findings.iter().find(|f| f.finding_id == finding_id)?;
    let (patch_hint, safe) = patch_hint_for_rule(&finding.rule_id, finding.file.as_deref());
    Some(FixProposal {
        finding_id: finding_id.to_string(),
        rule_id: finding.rule_id.clone(),
        file: finding.file.clone(),
        description: finding.suggested_fix.clone(),
        patch_hint,
        safe_to_auto_apply: safe,
    })
}

pub fn apply_safe_fix(finding: &DiagnosticFinding, dry_run: bool) -> ApplyFixResult {
    match finding.rule_id.as_str() {
        "pilcrow-load-not-async" => {
            if let Some(file) = &finding.file {
                if let Ok(source) = fs::read_to_string(file) {
                    let patched = source.replace("pub fn load(", "pub async fn load(");
                    if patched != source {
                        if !dry_run {
                            let _ = fs::write(file, &patched);
                        }
                        return ApplyFixResult {
                            finding_id: finding.finding_id.clone(),
                            applied: !dry_run,
                            dry_run,
                            message: "Replaced `pub fn load(` with `pub async fn load(`."
                                .to_string(),
                        };
                    }
                }
            }
        }
        _ => {}
    }
    ApplyFixResult {
        finding_id: finding.finding_id.clone(),
        applied: false,
        dry_run,
        message: format!(
            "No automatic fix available for rule '{}'. Apply manually: {}",
            finding.rule_id, finding.suggested_fix
        ),
    }
}

fn make_finding(
    counter: &mut usize,
    severity: Severity,
    rule_id: &str,
    message: &str,
    file: Option<&str>,
    line: Option<usize>,
    suggested_fix: &str,
) -> DiagnosticFinding {
    *counter += 1;
    let source_ref = rule_source_ref(rule_id);
    DiagnosticFinding {
        finding_id: format!("finding-{counter:04}"),
        severity,
        rule_id: rule_id.to_string(),
        message: message.to_string(),
        file: file.map(str::to_string),
        line,
        source_ref,
        suggested_fix: suggested_fix.to_string(),
    }
}

fn validation_finding_to_diag(counter: &mut usize, vf: Finding) -> DiagnosticFinding {
    *counter += 1;
    let source_ref = rule_source_ref(&vf.rule_id);
    DiagnosticFinding {
        finding_id: format!("finding-{counter:04}"),
        severity: vf.severity,
        rule_id: vf.rule_id,
        message: vf.message,
        file: vf.path,
        line: None,
        source_ref,
        suggested_fix: vf.suggested_fix.unwrap_or_default(),
    }
}

fn rule_source_ref(rule_id: &str) -> Option<String> {
    let refs: BTreeMap<&str, &str> = [
        (
            "pilcrow-load-not-async",
            "crates/routekit/src/templating/codegen/instrument.rs",
        ),
        (
            "pilcrow-load-async",
            "crates/routekit/src/templating/codegen/instrument.rs",
        ),
        (
            "pilcrow-load-return",
            "crates/routekit/src/templating/codegen/instrument.rs",
        ),
        (
            "pilcrow-api-missing-router",
            "crates/routekit/src/templating/codegen/api_routes.rs",
        ),
        (
            "pilcrow-missing-not-found",
            "crates/routekit/src/templating/codegen/app_module.rs",
        ),
        (
            "pilcrow-no-generated-artifacts",
            "crates/routekit/src/lib.rs",
        ),
        (
            "pilcrow-missing-generated-app",
            "crates/routekit/src/lib.rs",
        ),
        ("pilcrow-planned-islands", "registry.toml: feature islands"),
        (
            "pilcrow-planned-static-output",
            "registry.toml: feature ssg",
        ),
        (
            "pilcrow-boundary-silcrow-in-rust",
            "CLAUDE.md: silcrow.js section",
        ),
    ]
    .into_iter()
    .collect();
    refs.get(rule_id).map(|s| s.to_string())
}

fn patch_hint_for_rule(rule_id: &str, file: Option<&str>) -> (String, bool) {
    match rule_id {
        "pilcrow-load-not-async" => (
            format!(
                "In {}: change `pub fn load(` to `pub async fn load(`.",
                file.unwrap_or("the code-behind file")
            ),
            true,
        ),
        "pilcrow-missing-not-found" => (
            "Create pages/_not_found.html with a <h1>Not Found</h1> body.".to_string(),
            false,
        ),
        "pilcrow-api-missing-router" => (
            format!(
                "In {}: add `pub fn router() -> axum::Router {{ Router::new() }}`.",
                file.unwrap_or("the API file")
            ),
            false,
        ),
        "pilcrow-no-out-dir" | "pilcrow-no-generated-artifacts" => (
            "Run `cargo build --manifest-path sandbox/Cargo.toml` from the project root."
                .to_string(),
            false,
        ),
        _ => (
            "No automated patch available. Apply the suggested_fix manually.".to_string(),
            false,
        ),
    }
}

fn collect_html_routes(pages: &Path) -> Vec<String> {
    let mut routes = Vec::new();
    collect_html_recursive(pages, pages, &mut routes);
    routes
}

fn collect_html_recursive(base: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_html_recursive(base, &path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("html") {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !matches!(stem, "_layout" | "_loading" | "_error" | "_not_found") {
                let rel = path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                out.push(rel);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnose_sandbox_produces_result() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let result = diagnose_project(&root, None, None).unwrap();
        // Should at minimum report something (no panic)
        assert!(!result.scope.is_empty());
    }
}
