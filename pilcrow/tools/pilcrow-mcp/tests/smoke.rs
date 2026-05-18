/// MCP stdio smoke tests for Milestone 7.
///
/// Spawns the compiled `pilcrow-mcp` binary, drives it via the JSON-RPC 2.0
/// stdio transport, and verifies that:
///   - tools/list returns every expected tool
///   - resources/list returns every expected resource
///   - every resource can be read
///   - one tool from each capability group returns a valid response
///   - dry-run scaffolding works
///   - calling a planned/unsupported feature returns an error finding, not a panic
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

// ── Minimal MCP stdio client ──────────────────────────────────────────────────

struct McpClient {
    process: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpClient {
    /// Spawn the binary with cwd = Pilcrow repo root so it can locate
    /// `registry.toml`, `CLAUDE.md`, and the sandbox.
    fn spawn() -> Self {
        let bin = PathBuf::from(env!("CARGO_BIN_EXE_pilcrow-mcp"));
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        let mut process = Command::new(&bin)
            .current_dir(&repo_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", bin.display()));

        let stdin = process.stdin.take().unwrap();
        let stdout = process.stdout.take().unwrap();
        let reader = BufReader::new(stdout);

        let mut client = McpClient {
            process,
            stdin,
            reader,
            next_id: 1,
        };
        client.initialize();
        client
    }

    /// Send a JSON-RPC request and return the parsed response Value.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let mut line = serde_json::to_string(&msg).unwrap();
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).unwrap();
        self.stdin.flush().unwrap();
        self.read_response()
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    fn notify(&mut self, method: &str, params: Value) {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let mut line = serde_json::to_string(&msg).unwrap();
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).unwrap();
        self.stdin.flush().unwrap();
    }

    fn read_response(&mut self) -> Value {
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .expect("failed to read response line from MCP server");
        serde_json::from_str(line.trim())
            .unwrap_or_else(|e| panic!("failed to parse JSON response: {e}\nraw: {line}"))
    }

    fn initialize(&mut self) {
        let resp = self.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "smoke-test", "version": "0.1.0" }
            }),
        );
        assert_eq!(
            resp["jsonrpc"], "2.0",
            "initialize response should be JSON-RPC 2.0"
        );
        assert!(
            resp["error"].is_null(),
            "initialize should not error: {resp}"
        );
        self.notify("notifications/initialized", json!({}));
    }

    fn call_tool(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({ "name": name, "arguments": args }))
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

// ── Tools/list ────────────────────────────────────────────────────────────────

const EXPECTED_TOOLS: &[&str] = &[
    "list_features",
    "get_feature_spec",
    "explain_feature",
    "answer_pilcrow_question",
    "scan_project_context",
    "validate_implementation",
    "suggest_optimizations",
    "orchestrate_feature",
    "codegen_build",
    "codegen_list",
    "codegen_read",
    "inspect_route",
    "inspect_template",
    "inspect_code_behind",
    "inspect_generated_route",
    "compare_patterns",
    "why_build_failed",
    "diagnose_project",
    "diagnose_route",
    "diagnose_codegen",
    "propose_fix",
    "apply_safe_fix",
    "suggest_pattern",
];

#[test]
fn smoke_tools_list_contains_all_expected_tools() {
    let mut client = McpClient::spawn();
    let resp = client.request("tools/list", json!({}));

    assert!(
        resp["error"].is_null(),
        "tools/list should not error: {resp}"
    );
    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools/list result should have a 'tools' array");

    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    for expected in EXPECTED_TOOLS {
        assert!(
            names.contains(expected),
            "tool '{expected}' missing from tools/list; got: {names:?}"
        );
    }
}

// ── Resources/list ────────────────────────────────────────────────────────────

const EXPECTED_RESOURCE_URIS: &[&str] = &[
    "pilcrow://docs",
    "pilcrow://routekit/features",
    "pilcrow://current-project",
];

#[test]
fn smoke_resources_list_contains_all_expected_resources() {
    let mut client = McpClient::spawn();
    let resp = client.request("resources/list", json!({}));

    assert!(
        resp["error"].is_null(),
        "resources/list should not error: {resp}"
    );
    let resources = resp["result"]["resources"]
        .as_array()
        .expect("resources/list result should have a 'resources' array");

    let uris: Vec<&str> = resources.iter().filter_map(|r| r["uri"].as_str()).collect();

    for expected in EXPECTED_RESOURCE_URIS {
        assert!(
            uris.contains(expected),
            "resource '{expected}' missing from resources/list; got: {uris:?}"
        );
    }
}

// ── Read every resource ───────────────────────────────────────────────────────

#[test]
fn smoke_read_docs_resource() {
    let mut client = McpClient::spawn();
    let resp = client.request("resources/read", json!({ "uri": "pilcrow://docs" }));
    assert!(
        resp["error"].is_null(),
        "reading pilcrow://docs should not error: {resp}"
    );
    let contents = &resp["result"]["contents"];
    assert!(
        contents.is_array() && !contents.as_array().unwrap().is_empty(),
        "pilcrow://docs should return non-empty contents"
    );
}

#[test]
fn smoke_read_routekit_features_resource() {
    let mut client = McpClient::spawn();
    let resp = client.request(
        "resources/read",
        json!({ "uri": "pilcrow://routekit/features" }),
    );
    assert!(resp["error"].is_null(), "reading routekit features: {resp}");
}

#[test]
fn smoke_read_current_project_resource() {
    let mut client = McpClient::spawn();
    let resp = client.request(
        "resources/read",
        json!({ "uri": "pilcrow://current-project" }),
    );
    assert!(resp["error"].is_null(), "reading current-project: {resp}");
}

// ── One tool per capability group ─────────────────────────────────────────────

/// Knowledge / registry group
#[test]
fn smoke_list_features_returns_features() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool("list_features", json!({}));
    assert!(resp["error"].is_null(), "list_features errored: {resp}");
    let content = &resp["result"]["content"];
    assert!(
        content.is_array() && !content.as_array().unwrap().is_empty(),
        "list_features should return non-empty content"
    );
}
#[test]
fn smoke_list_features_surfaces_fsr_redis_requirements() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool("list_features", json!({ "domain": "pilcrow" }));
    assert!(resp["error"].is_null(), "list_features errored: {resp}");
    let body = resp.to_string();
    assert!(
        body.contains("\"id\":\"fsr\""),
        "missing fsr summary: {resp}"
    );
    assert!(
        body.contains("live-props-redis"),
        "fsr summary should mention live-props-redis: {resp}"
    );
    assert!(
        body.contains("Redis cache/pub-sub") || body.contains("Redis mode uses pub/sub"),
        "fsr summary should mention Redis pub/sub: {resp}"
    );
    assert!(
        body.contains("[fsr].redis_url"),
        "fsr summary should mention [fsr].redis_url: {resp}"
    );
}

/// Knowledge / registry group — feature spec
#[test]
fn smoke_get_feature_spec_for_ssr_pages() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool("get_feature_spec", json!({ "id": "ssr-pages" }));
    assert!(resp["error"].is_null(), "get_feature_spec errored: {resp}");
}

#[test]
fn smoke_get_feature_spec_for_fsr_keeps_redis_contract() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool("get_feature_spec", json!({ "id": "fsr" }));
    assert!(resp["error"].is_null(), "get_feature_spec errored: {resp}");
    let body = resp.to_string();
    for expected in [
        "live-props-redis",
        "Redis is the hot path",
        "Postgres is the truth layer",
        "disk is async recovery",
        "pilcrow:invalidate",
        "pilcrow:patch",
        "500ms polling",
        "[fsr]",
    ] {
        assert!(
            body.contains(expected),
            "fsr spec should include {expected:?}: {resp}"
        );
    }
}

/// Knowledge / registry group — React production pattern
#[test]
fn smoke_suggest_pattern_react_island_uses_hooks() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool(
        "suggest_pattern",
        json!({
            "description": "React 19 product island with hooks and action form",
            "include_code": true
        }),
    );
    assert!(resp["error"].is_null(), "suggest_pattern errored: {resp}");

    let text = serde_json::to_string(&resp["result"]).unwrap_or_default();
    assert!(
        text.contains("react-islands"),
        "missing React match: {text}"
    );
    assert!(
        text.contains("pilcrow/react"),
        "missing hook import: {text}"
    );
    assert!(
        text.contains("ProductPanel.jsx"),
        "missing JSX island example: {text}"
    );
}

/// Expert Q&A group
#[test]
fn smoke_answer_pilcrow_question_returns_answer() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool(
        "answer_pilcrow_question",
        json!({ "question": "How do I add nested layouts?" }),
    );
    assert!(
        resp["error"].is_null(),
        "answer_pilcrow_question errored: {resp}"
    );
    let content = &resp["result"]["content"];
    assert!(
        content.is_array() && !content.as_array().unwrap().is_empty(),
        "answer should be non-empty"
    );
}

/// Project scanning group
#[test]
fn smoke_scan_project_context_returns_context() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool("scan_project_context", json!({}));
    assert!(
        resp["error"].is_null(),
        "scan_project_context errored: {resp}"
    );
}

/// Validation group
#[test]
fn smoke_validate_valid_load_returns_valid() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool(
        "validate_implementation",
        json!({
            "code": "pub struct Props {}\npub async fn load(req: Req) -> AppResult<Props> { Ok(Props {}) }",
            "path": "src/pages/index.rs"
        }),
    );
    assert!(
        resp["error"].is_null(),
        "validate_implementation errored: {resp}"
    );
}

/// Scaffolding group — dry run
#[test]
fn smoke_orchestrate_dry_run_loaded_page() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool(
        "orchestrate_feature",
        json!({
            "kind": "loaded-page",
            "name": "smoke-test-page",
            "route_path": "/smoke",
            "dry_run": true
        }),
    );
    assert!(
        resp["error"].is_null(),
        "orchestrate_feature (dry-run) errored: {resp}"
    );
    // The result content should mention at least one file
    let text = serde_json::to_string(&resp["result"]).unwrap_or_default();
    assert!(
        text.contains("smoke-test-page") || text.contains("smoke"),
        "dry-run result should mention the page name; got: {text}"
    );
}

/// Codegen group
#[test]
fn smoke_codegen_list_returns_result() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool("codegen_list", json!({}));
    // codegen_list may fail if the sandbox hasn't been built, but it must not panic
    // and must return a valid JSON-RPC response either way
    assert!(
        resp["error"].is_null() || resp["result"].is_object(),
        "codegen_list should return a structured response"
    );
}

/// Deep inspection group
#[test]
fn smoke_inspect_route_returns_inspection() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool("inspect_route", json!({ "route": "/" }));
    // Inspection may return a not-found error for the route if sandbox hasn't been built;
    // what we care about is that the binary handles it without crashing
    assert!(
        !resp["result"].is_null() || !resp["error"].is_null(),
        "inspect_route should return either a result or an MCP error"
    );
}

/// Diagnostics group
#[test]
fn smoke_diagnose_project_returns_result() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool("diagnose_project", json!({}));
    assert!(resp["error"].is_null(), "diagnose_project errored: {resp}");
}

// ── Dry-run vs write-mode scaffolding ─────────────────────────────────────────

#[test]
fn smoke_orchestrate_defaults_to_dry_run() {
    let mut client = McpClient::spawn();
    // No dry_run param — should default to true and not write files
    let resp = client.call_tool(
        "orchestrate_feature",
        json!({ "kind": "static-page", "name": "default-dry" }),
    );
    assert!(
        resp["error"].is_null(),
        "orchestrate_feature errored: {resp}"
    );
    let text = serde_json::to_string(&resp["result"]).unwrap_or_default();
    // Either "dry_run":true or "action":"dry_run" should appear
    assert!(
        text.contains("dry"),
        "default scaffold should indicate dry-run mode; got: {text}"
    );
}

// ── Error responses for planned/unsupported features ─────────────────────────

#[test]
fn smoke_validate_island_directive_returns_error_finding() {
    let mut client = McpClient::spawn();
    let resp = client.call_tool(
        "validate_implementation",
        json!({
            "code": "<Island client:load />",
            "path": "src/pages/index.html"
        }),
    );
    assert!(
        resp["error"].is_null(),
        "validate_implementation itself should not error"
    );
    let text = serde_json::to_string(&resp["result"]).unwrap_or_default();
    assert!(
        text.contains("planned") || text.contains("island") || text.contains("Island"),
        "result should mention planned islands; got: {text}"
    );
}

#[test]
fn smoke_validate_prerender_const_is_accepted() {
    // PRERENDER is now a stable SSG feature — pub const PRERENDER: bool = true is valid.
    let mut client = McpClient::spawn();
    let resp = client.call_tool(
        "validate_implementation",
        json!({
            "code": "pub const PRERENDER: bool = true;",
            "path": "src/pages/index.rs"
        }),
    );
    assert!(resp["error"].is_null(), "validate_implementation errored");
    let text = serde_json::to_string(&resp["result"]).unwrap_or_default();
    // Valid SSG declaration should produce no error-level findings.
    assert!(
        text.contains("\"valid\":true") || text.contains("findings\":[]"),
        "PRERENDER = true should be accepted as valid SSG syntax; got: {text}"
    );
}

#[test]
fn smoke_unknown_tool_returns_json_rpc_error() {
    let mut client = McpClient::spawn();
    let resp = client.request(
        "tools/call",
        json!({ "name": "nonexistent_tool", "arguments": {} }),
    );
    // rmcp should return a JSON-RPC error (method not found or unknown tool)
    assert!(
        !resp["error"].is_null(),
        "calling an unknown tool should return a JSON-RPC error"
    );
}
