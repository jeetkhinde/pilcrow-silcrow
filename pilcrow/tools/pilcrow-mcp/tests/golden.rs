/// Fixture-backed golden tests for Milestone 7.
///
/// Each test builds a minimal in-memory project tree with tempfile, then
/// calls the domain functions directly and asserts on the output shape.
/// No binary is required — pure library tests.
use pilcrow_mcp::{
    diagnostics::diagnose_project,
    validation::validate_implementation,
    workspace::{parse_code_behind, scan_project},
};
use std::fs;
use tempfile::TempDir;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn pilcrow_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Creates a minimal `{dir}/{name}/` project with a bare Cargo.toml.
fn minimal_app(dir: &TempDir, name: &str) -> std::path::PathBuf {
    let app = dir.path().join(name);
    fs::create_dir_all(&app).unwrap();
    fs::write(
        app.join("Cargo.toml"),
        format!("[package]\nname=\"{name}\"\nversion=\"0.1.0\"\nedition=\"2021\"\n"),
    )
    .unwrap();
    app
}

fn scan(dir: &TempDir, app: &str) -> pilcrow_mcp::workspace::ProjectContext {
    scan_project(
        dir.path(),
        Some(dir.path().to_str().unwrap()),
        Some(&format!("{app}/Cargo.toml")),
    )
    .unwrap()
}

// ── Route graph extraction ────────────────────────────────────────────────────

#[test]
fn route_graph_static_routes() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/pages")).unwrap();
    fs::write(app.join("src/pages/index.html"), "<h1>Home</h1>").unwrap();
    fs::write(app.join("src/pages/about.html"), "<h1>About</h1>").unwrap();

    let ctx = scan(&dir, "app");
    let patterns: Vec<&str> = ctx
        .route_graph
        .iter()
        .map(|n| n.url_pattern.as_str())
        .collect();
    assert!(patterns.contains(&"/"), "missing / route; got {patterns:?}");
    assert!(
        patterns.contains(&"/about"),
        "missing /about route; got {patterns:?}"
    );
}

#[test]
fn route_graph_respects_ignored_directories() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/pages/Cards")).unwrap();
    fs::write(
        app.join("Pilcrow.toml"),
        "[routing]\nignore_directories = [\"Cards\"]\n",
    )
    .unwrap();
    fs::write(app.join("src/pages/index.html"), "<h1>Home</h1>").unwrap();
    fs::write(app.join("src/pages/Cards/Card.html"), "<h1>Card</h1>").unwrap();

    let ctx = scan(&dir, "app");
    let files: Vec<&str> = ctx.routes.iter().map(|route| route.path.as_str()).collect();
    assert!(files.contains(&"index.html"), "missing index route");
    assert!(
        !files.contains(&"Cards/Card.html"),
        "ignored directory leaked into route scan: {files:?}"
    );
}

#[test]
fn route_graph_dynamic_route() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/pages/[id]")).unwrap();
    fs::write(app.join("src/pages/[id]/index.html"), "<h1>Item</h1>").unwrap();

    let ctx = scan(&dir, "app");
    let node = ctx
        .route_graph
        .iter()
        .find(|n| n.is_dynamic)
        .expect("expected a dynamic route node");
    assert_eq!(node.url_pattern, "/:id");
    assert!(node.dynamic_params.contains(&"id".to_string()));
}

#[test]
fn route_graph_constraint_param_strips_matcher() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/pages/[id=integer]")).unwrap();
    fs::write(
        app.join("src/pages/[id=integer]/index.html"),
        "<h1>Int</h1>",
    )
    .unwrap();

    let ctx = scan(&dir, "app");
    let node = ctx
        .route_graph
        .iter()
        .find(|n| n.is_dynamic)
        .expect("expected dynamic route for constrained param");
    assert_eq!(
        node.url_pattern, "/:id",
        "constraint suffix should be stripped from URL pattern"
    );
}

#[test]
fn route_graph_catch_all_route() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/pages/[...rest]")).unwrap();
    fs::write(app.join("src/pages/[...rest]/index.html"), "").unwrap();

    let ctx = scan(&dir, "app");
    let node = ctx
        .route_graph
        .iter()
        .find(|n| n.url_pattern.contains("*rest"))
        .expect("expected catch-all route");
    assert_eq!(node.url_pattern, "/*rest");
}

// ── Layout chains ─────────────────────────────────────────────────────────────

#[test]
fn layout_chain_single_level() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/pages")).unwrap();
    fs::write(app.join("src/pages/_layout.html"), "<slot />").unwrap();
    fs::write(app.join("src/pages/index.html"), "<h1>Home</h1>").unwrap();

    let ctx = scan(&dir, "app");
    let node = ctx
        .route_graph
        .iter()
        .find(|n| n.url_pattern == "/")
        .unwrap();
    assert!(
        !node.layout_chain.is_empty(),
        "expected root layout in chain"
    );
    assert!(node.layout_chain.iter().any(|l| l.contains("_layout")));
}

#[test]
fn layout_chain_nested_directories() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/pages/admin")).unwrap();
    fs::write(app.join("src/pages/_layout.html"), "<slot />").unwrap();
    fs::write(app.join("src/pages/admin/_layout.html"), "<slot />").unwrap();
    fs::write(app.join("src/pages/admin/index.html"), "<h1>Admin</h1>").unwrap();

    let ctx = scan(&dir, "app");
    let node = ctx
        .route_graph
        .iter()
        .find(|n| n.url_pattern == "/admin")
        .unwrap();
    assert!(
        node.layout_chain.len() >= 2,
        "expected both root and admin layouts; got {:?}",
        node.layout_chain
    );
}

#[test]
fn route_group_strips_name_from_url() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/pages/(admin)")).unwrap();
    fs::write(app.join("src/pages/(admin)/_layout.html"), "<slot />").unwrap();
    fs::write(
        app.join("src/pages/(admin)/dashboard.html"),
        "<h1>Dashboard</h1>",
    )
    .unwrap();

    let ctx = scan(&dir, "app");
    // Route group `(admin)` must be stripped — URL is /dashboard, not /admin/dashboard
    let node = ctx
        .route_graph
        .iter()
        .find(|n| n.url_pattern == "/dashboard")
        .expect("expected /dashboard route with group name stripped");
    assert!(
        node.layout_chain.iter().any(|l| l.contains("_layout")),
        "expected group layout in chain"
    );
}

#[test]
fn loading_skeleton_detected_in_chain() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/pages")).unwrap();
    fs::write(app.join("src/pages/_loading.html"), "<p>Loading…</p>").unwrap();
    fs::write(app.join("src/pages/index.html"), "<h1>Home</h1>").unwrap();

    let ctx = scan(&dir, "app");
    let node = ctx
        .route_graph
        .iter()
        .find(|n| n.url_pattern == "/")
        .unwrap();
    assert!(
        node.has_loading,
        "expected has_loading when _loading.html present"
    );
}

#[test]
fn error_page_detected_in_chain() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/pages")).unwrap();
    fs::write(app.join("src/pages/_error.html"), "<p>Error</p>").unwrap();
    fs::write(app.join("src/pages/index.html"), "<h1>Home</h1>").unwrap();

    let ctx = scan(&dir, "app");
    let node = ctx
        .route_graph
        .iter()
        .find(|n| n.url_pattern == "/")
        .unwrap();
    assert!(
        node.has_error,
        "expected has_error when _error.html present"
    );
    assert!(ctx.has_global_error, "expected has_global_error flag set");
}

// ── Invalid load functions ────────────────────────────────────────────────────

#[test]
fn invalid_load_non_async_rejected() {
    let code = "pub struct Props {}\npub fn load(req: Req) -> AppResult<Props> { Ok(Props {}) }";
    let report = validate_implementation(code, Some("src/pages/index.rs"), None);
    assert!(!report.valid);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-load-async"),
        "expected pilcrow-load-async finding"
    );
}

#[test]
fn invalid_load_wrong_return_type_rejected() {
    let code = "pub struct Props {}\npub async fn load(req: Req) -> Result<Props, String> { Ok(Props {}) }";
    let report = validate_implementation(code, Some("src/pages/index.rs"), None);
    assert!(!report.valid);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-load-return"),
        "expected pilcrow-load-return finding"
    );
}

#[test]
fn invalid_load_missing_req_warns() {
    let code = "pub struct Props {}\npub async fn load() -> AppResult<Props> { Ok(Props {}) }";
    let report = validate_implementation(code, Some("src/pages/index.rs"), None);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-load-req"),
        "expected pilcrow-load-req warning"
    );
}

#[test]
fn valid_load_shape_accepted() {
    let code =
        "pub struct Props {}\npub async fn load(req: Req) -> AppResult<Props> { Ok(Props {}) }";
    let report = validate_implementation(code, Some("src/pages/index.rs"), None);
    assert!(
        report.valid,
        "valid load signature should pass; findings: {:?}",
        report.findings
    );
}

// ── Invalid named actions ─────────────────────────────────────────────────────

#[test]
fn invalid_action_wrong_return_type_warns() {
    let code = "pub async fn submit(req: Req) -> String { \"ok\".to_string() }";
    let report = validate_implementation(code, Some("src/pages/index.rs"), None);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-action-return"),
        "expected pilcrow-action-return for action with wrong return type"
    );
}

#[test]
fn valid_action_in_fragment_accepted() {
    let code = "pub async fn refresh(req: Req) -> ActionResult { redirect(\"/\") }";
    let report = validate_implementation(code, Some("src/widgets/user-card.rs"), None);
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.rule_id != "pilcrow-component-no-actions"
                && f.rule_id != "pilcrow-layout-no-actions"
                && f.rule_id != "pilcrow-action-return"),
        "fragment action should be accepted; findings: {:?}",
        report.findings
    );
}

#[test]
fn invalid_action_in_layout_rejected() {
    let code = "pub async fn save(req: Req) -> ActionResult { redirect(\"/\") }";
    let report = validate_implementation(code, Some("src/pages/_layout.rs"), None);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-layout-no-actions"),
        "expected pilcrow-layout-no-actions for action in _layout.rs"
    );
}

#[test]
fn invalid_action_in_ui_component_rejected() {
    let code = "pub async fn click(req: Req) -> ActionResult { redirect(\"/\") }";
    let report = validate_implementation(code, Some("src/ui/Button.rs"), None);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-component-no-actions"),
        "expected pilcrow-component-no-actions for action in UI component"
    );
}

// ── Bad template imports / HTML validation ────────────────────────────────────

#[test]
fn island_directive_in_template_rejected() {
    let code = "<Island client:load src=\"./Counter.js\" />";
    let report = validate_implementation(code, Some("src/pages/index.html"), None);
    assert!(!report.valid);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-wrong-island-syntax"),
        "expected pilcrow-wrong-island-syntax for <Island> tag"
    );
}

#[test]
fn client_idle_directive_in_template_rejected() {
    let code = "<div client:idle>Widget</div>";
    let report = validate_implementation(code, Some("src/pages/index.html"), None);
    assert!(!report.valid);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-wrong-island-syntax"),
        "expected pilcrow-wrong-island-syntax for client:idle"
    );
}

#[test]
fn ssg_keyword_in_template_rejected() {
    let code = "<!-- generateStaticParams -->\n<p>static</p>";
    let report = validate_implementation(code, Some("src/pages/index.html"), None);
    assert!(!report.valid);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-planned-static-output"),
        "expected pilcrow-planned-static-output for generateStaticParams"
    );
}

#[test]
fn client_only_state_hook_warns() {
    let code = "<script>const [x] = useState(0)</script>";
    let report = validate_implementation(code, Some("src/pages/index.html"), None);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-hydration-static-mismatch"),
        "expected hydration warning for useState()"
    );
}

#[test]
fn island_finding_has_line_number() {
    let code = "line1\n<Island client:load />\nline3";
    let report = validate_implementation(code, Some("src/pages/index.html"), None);
    let finding = report
        .findings
        .iter()
        .find(|f| f.rule_id == "pilcrow-wrong-island-syntax")
        .expect("expected island finding");
    assert_eq!(finding.line, Some(2), "expected line 2 for the Island tag");
}

// ── Fragment config edge cases ────────────────────────────────────────────────

#[test]
fn scan_detects_fragment_directory() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/widgets")).unwrap();
    fs::write(app.join("src/widgets/user-card.html"), "<p>Card</p>").unwrap();
    fs::write(
        app.join("Pilcrow.toml"),
        "[[fragments]]\ndir = \"widgets\"\n",
    )
    .unwrap();

    let ctx = scan(&dir, "app");
    assert!(
        !ctx.fragments.is_empty(),
        "expected fragment group to be detected"
    );
    let frag = &ctx.fragments[0];
    assert_eq!(frag.url, "widgets");
    assert!(
        frag.files.iter().any(|f| f.path.contains("user-card")),
        "expected user-card in fragment files"
    );
}

#[test]
fn scan_fragment_explicit_url_override() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/ui-blocks")).unwrap();
    fs::write(
        app.join("src/ui-blocks/hero.html"),
        "<section>Hero</section>",
    )
    .unwrap();
    fs::write(
        app.join("Pilcrow.toml"),
        "[[fragments]]\ndir = \"ui-blocks\"\nurl = \"blocks\"\n",
    )
    .unwrap();

    let ctx = scan(&dir, "app");
    let frag = &ctx.fragments[0];
    assert_eq!(
        frag.url, "blocks",
        "explicit url override should be respected"
    );
}

#[test]
fn scan_multiple_fragment_directories() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/widgets")).unwrap();
    fs::create_dir_all(app.join("src/partials")).unwrap();
    fs::write(app.join("src/widgets/card.html"), "").unwrap();
    fs::write(app.join("src/partials/footer.html"), "").unwrap();
    fs::write(
        app.join("Pilcrow.toml"),
        "[[fragments]]\ndir = \"widgets\"\n\n[[fragments]]\ndir = \"partials\"\n",
    )
    .unwrap();

    let ctx = scan(&dir, "app");
    assert_eq!(
        ctx.fragments.len(),
        2,
        "expected two fragment groups to be detected"
    );
}

#[test]
fn scan_empty_fragment_directory_still_registered() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/chunks")).unwrap();
    fs::write(
        app.join("Pilcrow.toml"),
        "[[fragments]]\ndir = \"chunks\"\n",
    )
    .unwrap();

    let ctx = scan(&dir, "app");
    assert_eq!(
        ctx.fragments.len(),
        1,
        "empty fragment dir should still be in the fragment list"
    );
    assert!(ctx.fragments[0].files.is_empty());
}

// ── Deferred fields ───────────────────────────────────────────────────────────

#[test]
fn code_behind_detects_deferred_field() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("home.rs");
    fs::write(
        &path,
        r#"
pub struct Props {
    pub title: String,
    pub count: Deferred<i32>,
}
pub async fn load(_req: Req) -> AppResult<Props> { unimplemented!() }
"#,
    )
    .unwrap();

    let info = parse_code_behind(&path).unwrap();
    assert!(info.has_deferred, "expected has_deferred flag");
    let count = info
        .prop_fields
        .iter()
        .find(|f| f.name == "count")
        .expect("expected 'count' field");
    assert!(count.is_deferred, "count should be marked is_deferred");
}

#[test]
fn code_behind_detects_deferred_html_field() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shop.rs");
    fs::write(
        &path,
        r#"
pub struct Props {
    pub title: String,
    pub product_list: DeferredHtml,
}
pub async fn load(_req: Req) -> AppResult<Props> { unimplemented!() }
"#,
    )
    .unwrap();

    let info = parse_code_behind(&path).unwrap();
    assert!(info.has_deferred, "expected has_deferred for DeferredHtml");
    let field = info
        .prop_fields
        .iter()
        .find(|f| f.name == "product_list")
        .expect("expected 'product_list' field");
    assert!(field.is_deferred);
}

#[test]
fn code_behind_non_deferred_props_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("static.rs");
    fs::write(
        &path,
        r#"
pub struct Props { pub title: String, pub count: i32 }
pub async fn load(_req: Req) -> AppResult<Props> { unimplemented!() }
"#,
    )
    .unwrap();

    let info = parse_code_behind(&path).unwrap();
    assert!(
        !info.has_deferred,
        "non-deferred Props should not set has_deferred"
    );
}

// ── Middleware detection ──────────────────────────────────────────────────────

#[test]
fn scan_detects_middleware_file() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::write(
        app.join("src/middleware.rs"),
        "pub async fn middleware(req: Req, next: Next) -> Response { next.run().await }",
    )
    .unwrap();

    let ctx = scan(&dir, "app");
    assert!(
        ctx.middleware.is_some(),
        "expected middleware to be detected when src/middleware.rs exists"
    );
}

#[test]
fn scan_no_middleware_when_file_absent() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src")).unwrap();

    let ctx = scan(&dir, "app");
    assert!(
        ctx.middleware.is_none(),
        "expected no middleware when src/middleware.rs is absent"
    );
}

// ── API routes ────────────────────────────────────────────────────────────────

#[test]
fn scan_detects_api_routes() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src/api")).unwrap();
    fs::write(
        app.join("src/api/health.rs"),
        "pub fn router() -> axum::Router { axum::Router::new() }",
    )
    .unwrap();
    fs::write(
        app.join("src/api/users.rs"),
        "pub fn router() -> axum::Router { axum::Router::new() }",
    )
    .unwrap();

    let ctx = scan(&dir, "app");
    assert_eq!(ctx.api_routes.len(), 2, "expected two API routes");
    assert!(ctx.api_routes.iter().any(|r| r.path.contains("health")));
    assert!(ctx.api_routes.iter().any(|r| r.path.contains("users")));
}

#[test]
fn scan_no_api_routes_when_dir_absent() {
    let dir = tempfile::tempdir().unwrap();
    let app = minimal_app(&dir, "app");
    fs::create_dir_all(app.join("src")).unwrap();

    let ctx = scan(&dir, "app");
    assert!(
        ctx.api_routes.is_empty(),
        "expected no API routes when src/api/ is absent"
    );
}

// ── Generated-code inspection (uses real sandbox) ─────────────────────────────

#[test]
fn diagnose_sandbox_produces_non_empty_scope() {
    let root = pilcrow_root();
    let result = diagnose_project(&root, None, None).unwrap();
    assert!(
        !result.scope.is_empty(),
        "diagnose_project should return a non-empty scope for the sandbox"
    );
}

// ── Silcrow boundary / delegation ─────────────────────────────────────────────

#[test]
fn silcrow_directive_in_rust_flagged() {
    // s-boost is a silcrow.js directive and must not appear in Rust code
    let code = "fn setup() { println!(\"s-boost\"); }";
    let report = validate_implementation(code, Some("src/pages/index.rs"), None);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-boundary-silcrow-in-rust"),
        "expected boundary finding for silcrow directive in Rust"
    );
}

#[test]
fn client_island_in_rs_file_flagged() {
    let code = "fn x() { let _ = \"<Island\"; }";
    let report = validate_implementation(code, Some("src/pages/thing.rs"), None);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-boundary-silcrow-in-rust"),
        "expected boundary finding for <Island in Rust"
    );
}

#[test]
fn prerender_const_in_rust_accepted() {
    // PRERENDER is now a recognized ISR constant — routekit strips it from emitted code.
    // Build-time pre-warming (SSG) is not yet active, but the constant causes no error.
    let code = "pub const PRERENDER: bool = true;";
    let report = validate_implementation(code, Some("src/pages/index.rs"), None);
    // The constant should not produce an error-level finding.
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.rule_id == "pilcrow-planned-static-output"
                && f.severity == pilcrow_mcp::validation::Severity::Error),
        "PRERENDER should not produce an error-level finding now that ISR is implemented"
    );
}

#[test]
fn finding_has_source_ref() {
    let code = "pub fn load(req: Req) -> AppResult<Props> { Ok(Props {}) }";
    let report = validate_implementation(code, Some("src/pages/index.rs"), None);
    let finding = report
        .findings
        .iter()
        .find(|f| f.rule_id == "pilcrow-load-async")
        .expect("expected pilcrow-load-async finding");
    assert!(
        finding.source_ref.is_some(),
        "finding should include a source_ref"
    );
}
