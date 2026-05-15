use crate::{
    codegen, diagnostics,
    docs::{self, KnowledgeBase},
    inspect,
    registry::{FeatureDomain, FeatureStatus, Registry},
    scaffold::{orchestrate_feature, ScaffoldRequest},
    validation::validate_implementation,
    workspace::scan_project,
};
use anyhow::{Context, Result};
use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars::JsonSchema,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

const CURRENT_PROJECT_URI: &str = "pilcrow://current-project";

#[derive(Clone)]
pub struct PilcrowServer {
    project_root: PathBuf,
    registry: Registry,
    knowledge: KnowledgeBase,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

// ── Existing tool arg structs ──────────────────────────────────────────────

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ListFeaturesArgs {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FeatureSpecArgs {
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExplainFeatureArgs {
    pub id: String,
    #[serde(default)]
    pub depth: Option<String>,
    #[serde(default)]
    pub include_examples: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindExamplesArgs {
    pub feature: String,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnswerQuestionArgs {
    pub question: String,
    #[serde(default)]
    pub project_root: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ProjectArgs {
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct ValidateArgs {
    pub code: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub project_root: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SuggestArgs {
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
    #[serde(default)]
    pub focus: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct OrchestrateArgs {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub route_path: Option<String>,
    #[serde(default)]
    pub target_dir: Option<String>,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
    #[serde(default)]
    pub options: Option<Value>,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub overwrite: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct CodegenBuildArgs {
    #[serde(default)]
    pub manifest: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct CodegenListArgs {
    #[serde(default)]
    pub manifest: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CodegenReadArgs {
    pub file: String,
    #[serde(default)]
    pub manifest: Option<String>,
    #[serde(default)]
    pub head: Option<usize>,
    #[serde(default)]
    pub tail: Option<usize>,
}

// ── New tool arg structs ───────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InspectRouteArgs {
    pub route: String,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InspectTemplateArgs {
    pub path: String,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InspectCodeBehindArgs {
    pub path: String,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InspectGeneratedRouteArgs {
    pub route: String,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ComparePatternArgs {
    pub goal: String,
    #[serde(default)]
    pub options: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SuggestPatternArgs {
    /// Natural-language description of what you want to build.
    /// Examples: "page with 60s cache", "real-time counter with SSE",
    /// "product listing with deferred reviews", "action that updates a product"
    pub description: String,
    #[serde(default)]
    pub include_code: Option<bool>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct WhyBuildFailedArgs {
    #[serde(default)]
    pub manifest: Option<String>,
    #[serde(default)]
    pub error_log: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct DiagnoseProjectArgs {
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DiagnoseRouteArgs {
    pub route: String,
    #[serde(default)]
    pub project_root: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct DiagnoseCodegenArgs {
    #[serde(default)]
    pub manifest: Option<String>,
    #[serde(default)]
    pub project_root: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProposeFixArgs {
    pub finding_id: String,
    pub findings_json: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApplySafeFixArgs {
    pub finding_id: String,
    pub findings_json: String,
    #[serde(default)]
    pub dry_run: Option<bool>,
}

// ── Supporting output types ────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct Optimizations {
    focus: Option<String>,
    recommendations: Vec<Optimization>,
}

#[derive(Debug, Serialize)]
struct Optimization {
    rule_id: &'static str,
    severity: &'static str,
    message: String,
    suggested_fix: String,
}

#[derive(Debug, Serialize)]
struct PatternComparison {
    goal: String,
    patterns: Vec<Pattern>,
    recommendation: String,
}

#[derive(Debug, Serialize)]
struct Pattern {
    name: String,
    description: String,
    tradeoffs: Vec<String>,
    scaffold_kind: Option<String>,
    status: String,
}

// ── Tool implementations ───────────────────────────────────────────────────

#[tool_router]
impl PilcrowServer {
    pub fn new() -> Result<Self> {
        let cwd = std::env::current_dir().context("failed to read current directory")?;
        let project_root = crate::registry::find_project_root(&cwd).unwrap_or(cwd);
        let registry = Registry::load_from_project(&project_root)?;
        let knowledge = KnowledgeBase::load(&project_root)?;
        Ok(Self {
            project_root,
            registry,
            knowledge,
            tool_router: Self::tool_router(),
        })
    }

    #[tool(
        description = "List Pilcrow/Silcrow feature registry entries, optionally filtered by status or domain."
    )]
    async fn list_features(
        &self,
        Parameters(args): Parameters<ListFeaturesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let status = match args.status.as_deref().map(parse_status).transpose() {
            Ok(status) => status,
            Err(error) => return Ok(tool_error(error)),
        };
        let domain = match args.domain.as_deref().map(parse_domain).transpose() {
            Ok(domain) => domain,
            Err(error) => return Ok(tool_error(error)),
        };
        Ok(structured(self.registry.filtered(status, domain)))
    }

    #[tool(
        description = "Return one full Pilcrow/Silcrow feature spec with validation/scaffolding constraints, canonical usage, invalid examples, and source/test references."
    )]
    async fn get_feature_spec(
        &self,
        Parameters(args): Parameters<FeatureSpecArgs>,
    ) -> Result<CallToolResult, McpError> {
        match self.registry.feature(&args.id) {
            Some(feature) => Ok(structured(feature)),
            None => Ok(tool_error(format!("unknown feature id: {}", args.id))),
        }
    }

    #[tool(
        description = "Explain a Pilcrow feature using registry status plus local docs/tests/examples evidence."
    )]
    async fn explain_feature(
        &self,
        Parameters(args): Parameters<ExplainFeatureArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut explanation = match self.knowledge.explain_feature(&self.registry, &args.id) {
            Some(explanation) => explanation,
            None => return Ok(tool_error(format!("unknown feature id: {}", args.id))),
        };
        if args.include_examples == Some(false) {
            explanation
                .evidence
                .retain(|item| !item.path.starts_with("sandbox/"));
        }
        if matches!(args.depth.as_deref(), Some("brief")) {
            explanation.evidence.truncate(3);
        }
        Ok(structured(explanation))
    }

    #[tool(
        description = "Find sandbox examples and tests for a Pilcrow feature or implementation pattern."
    )]
    async fn find_examples(
        &self,
        Parameters(args): Parameters<FindExamplesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let limit = args.limit.unwrap_or(8).clamp(1, 20);
        Ok(structured(self.knowledge.find_examples(
            &args.feature,
            args.pattern.as_deref(),
            limit,
        )))
    }

    #[tool(
        description = "Answer a Pilcrow framework question from local registry/docs/tests/examples and delegate exact silcrow.js runtime questions to silcrow-mcp."
    )]
    async fn answer_pilcrow_question(
        &self,
        Parameters(args): Parameters<AnswerQuestionArgs>,
    ) -> Result<CallToolResult, McpError> {
        if args
            .project_root
            .as_deref()
            .is_some_and(|root| self.project_root.to_string_lossy().as_ref().ne(root))
        {
            return Ok(tool_error(
                "project_root-specific answer indexing is not implemented yet; this server currently answers from its loaded project root.",
            ));
        }
        Ok(structured(
            self.knowledge
                .answer_question(&self.registry, &args.question),
        ))
    }

    #[tool(
        description = "Scan a Pilcrow project for routes, layouts, components, fragments, APIs, params, middleware, config, crate versions, OUT_DIR status, and semantic route graph with URL patterns and code-behind metadata."
    )]
    async fn scan_project_context(
        &self,
        Parameters(args): Parameters<ProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match scan_project(
                &self.project_root,
                args.project_root.as_deref(),
                args.manifest_path.as_deref(),
            ) {
                Ok(context) => structured(context),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Validate Rust or HTML snippets against current Pilcrow/Silcrow conventions and planned-feature gates. Returns findings with severity, rule_id, line numbers, source references, and suggested fixes."
    )]
    async fn validate_implementation(
        &self,
        Parameters(args): Parameters<ValidateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let report =
            validate_implementation(&args.code, args.path.as_deref(), args.kind.as_deref());
        Ok(structured(report))
    }

    #[tool(
        description = "Analyze the current project map and recommend Pilcrow/Silcrow architecture improvements."
    )]
    async fn suggest_optimizations(
        &self,
        Parameters(args): Parameters<SuggestArgs>,
    ) -> Result<CallToolResult, McpError> {
        let context = match scan_project(
            &self.project_root,
            args.project_root.as_deref(),
            args.manifest_path.as_deref(),
        ) {
            Ok(context) => context,
            Err(error) => return Ok(tool_error(error.to_string())),
        };
        Ok(structured(suggest_from_context(&context, args.focus)))
    }

    #[tool(
        description = "Scaffold a Pilcrow route, component, fragment, layout, middleware, API route, param matcher, or env config. Supported kinds: route, static-page, loaded-page, action-page, deferred-page, component, fragment, silcrow-form, nested-layout, loading-page, error-page, not-found-page, api-route, middleware, env-config, typed-param. Defaults to dry-run."
    )]
    async fn orchestrate_feature(
        &self,
        Parameters(args): Parameters<OrchestrateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let request = ScaffoldRequest {
            kind: &args.kind,
            name: &args.name,
            route_path: args.route_path.as_deref(),
            target_dir: args.target_dir.as_deref(),
            options: args.options.as_ref(),
            dry_run: args.dry_run.unwrap_or(true),
            overwrite: args.overwrite.unwrap_or(false),
        };
        Ok(
            match orchestrate_feature(
                &self.project_root,
                args.project_root.as_deref(),
                args.manifest_path.as_deref(),
                request,
            ) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Build a Pilcrow web app and report generated routekit OUT_DIR artifacts."
    )]
    async fn codegen_build(
        &self,
        Parameters(args): Parameters<CodegenBuildArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match codegen::codegen_build(&self.project_root, args.manifest.as_deref()) {
                Ok(result) => {
                    if result.success {
                        structured(result)
                    } else {
                        CallToolResult::structured_error(json!(result))
                    }
                }
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(description = "List files generated by the Pilcrow routekit build pipeline in OUT_DIR.")]
    async fn codegen_list(
        &self,
        Parameters(args): Parameters<CodegenListArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match codegen::codegen_list(&self.project_root, args.manifest.as_deref()) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Read a generated OUT_DIR file, optionally limited with head or tail line counts."
    )]
    async fn codegen_read(
        &self,
        Parameters(args): Parameters<CodegenReadArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match codegen::codegen_read(
                &self.project_root,
                args.manifest.as_deref(),
                &args.file,
                args.head,
                args.tail,
            ) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Inspect a single Pilcrow route deeply: HTML template, code-behind metadata (Props, load, actions, deferred fields), layout chain, loading/error coverage, and a hint to the generated handler."
    )]
    async fn inspect_route(
        &self,
        Parameters(args): Parameters<InspectRouteArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match inspect::inspect_route(
                &self.project_root,
                args.project_root.as_deref(),
                args.manifest_path.as_deref(),
                &args.route,
            ) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Parse a Pilcrow HTML template for component imports, slot usages, Silcrow directives, and fragment slots."
    )]
    async fn inspect_template(
        &self,
        Parameters(args): Parameters<InspectTemplateArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match inspect::inspect_template(
                &self.project_root,
                args.project_root.as_deref(),
                args.manifest_path.as_deref(),
                &args.path,
            ) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Parse a Pilcrow code-behind .rs file with syn: extract Props fields, load() signature, named action names, AsyncValue/AsyncHtml fields, and page option constants."
    )]
    async fn inspect_code_behind(
        &self,
        Parameters(args): Parameters<InspectCodeBehindArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match inspect::inspect_code_behind(
                &self.project_root,
                args.project_root.as_deref(),
                args.manifest_path.as_deref(),
                &args.path,
            ) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Find and show the generated OUT_DIR code for a Pilcrow route: excerpts from generated_app.rs and generated_routes.rs that reference the route."
    )]
    async fn inspect_generated_route(
        &self,
        Parameters(args): Parameters<InspectGeneratedRouteArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match inspect::inspect_generated_route(
                &self.project_root,
                args.project_root.as_deref(),
                args.manifest_path.as_deref(),
                &args.route,
            ) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Compare Pilcrow implementation patterns for a given goal and recommend the best approach. For example: compare patterns for 'server-side form handling' or 'streaming data to the client'."
    )]
    async fn compare_patterns(
        &self,
        Parameters(args): Parameters<ComparePatternArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(structured(compare_patterns_for(
            &args.goal,
            args.options.as_ref(),
        )))
    }

    #[tool(
        description = "Diagnose why a Pilcrow build failed. Pass a cargo error_log or let the tool run a fresh build. Returns categorized error analysis with suggested fixes."
    )]
    async fn why_build_failed(
        &self,
        Parameters(args): Parameters<WhyBuildFailedArgs>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(error_log) = &args.error_log {
            Ok(structured(analyse_build_error(error_log)))
        } else {
            // Run a fresh build and capture errors
            match codegen::codegen_build(&self.project_root, args.manifest.as_deref()) {
                Ok(result) if !result.success => {
                    let combined = format!("{}\n{}", result.stdout, result.stderr);
                    Ok(structured(analyse_build_error(&combined)))
                }
                Ok(result) => Ok(structured(json!({
                    "status": "build_succeeded",
                    "out_dir": result.out_dir,
                    "generated_files": result.generated_files,
                }))),
                Err(e) => Ok(tool_error(e.to_string())),
            }
        }
    }

    #[tool(
        description = "Run a comprehensive diagnostic scan of a Pilcrow project: checks routes, code-behind signatures, codegen status, missing special pages, and validation findings."
    )]
    async fn diagnose_project(
        &self,
        Parameters(args): Parameters<DiagnoseProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match diagnostics::diagnose_project(
                &self.project_root,
                args.project_root.as_deref(),
                args.manifest_path.as_deref(),
            ) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Diagnose a specific Pilcrow route: checks HTML template, code-behind signatures, AsyncValue/AsyncHtml fields, action definitions, and validation findings."
    )]
    async fn diagnose_route(
        &self,
        Parameters(args): Parameters<DiagnoseRouteArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match diagnostics::diagnose_route(
                &self.project_root,
                args.project_root.as_deref(),
                args.manifest_path.as_deref(),
                &args.route,
            ) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Diagnose the Pilcrow codegen pipeline: checks OUT_DIR, required generated files, and cross-checks source routes against generated_app.rs."
    )]
    async fn diagnose_codegen(
        &self,
        Parameters(args): Parameters<DiagnoseCodegenArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(
            match diagnostics::diagnose_codegen(
                &self.project_root,
                args.project_root.as_deref(),
                args.manifest.as_deref(),
            ) {
                Ok(result) => structured(result),
                Err(error) => tool_error(error.to_string()),
            },
        )
    }

    #[tool(
        description = "Propose a detailed fix for a specific diagnostic finding. Pass the finding_id and the full findings JSON from diagnose_project or diagnose_route."
    )]
    async fn propose_fix(
        &self,
        Parameters(args): Parameters<ProposeFixArgs>,
    ) -> Result<CallToolResult, McpError> {
        let findings: Vec<diagnostics::DiagnosticFinding> =
            match serde_json::from_str(&args.findings_json) {
                Ok(f) => f,
                Err(e) => return Ok(tool_error(format!("failed to parse findings_json: {e}"))),
            };
        match diagnostics::propose_fix(&findings, &args.finding_id) {
            Some(proposal) => Ok(structured(proposal)),
            None => Ok(tool_error(format!(
                "finding_id '{}' not found in provided findings",
                args.finding_id
            ))),
        }
    }

    #[tool(
        description = "Given a natural-language description of what to build, identify the relevant Pilcrow features and return a ready-to-use code skeleton. Use this before writing any page, action, or API route."
    )]
    async fn suggest_pattern(
        &self,
        Parameters(args): Parameters<SuggestPatternArgs>,
    ) -> Result<CallToolResult, McpError> {
        let desc = args.description.to_lowercase();
        let include_code = args.include_code.unwrap_or(true);

        // Detect which features apply based on keyword matching.
        let mut matched: Vec<(&str, &str)> = Vec::new(); // (feature_id, reason)

        if desc.contains("cache")
            || desc.contains("isr")
            || desc.contains("revalidat")
            || desc.contains("stale")
        {
            matched.push(("incremental-ssr", "caching / revalidation keywords"));
        }
        if desc.contains("prerender")
            || desc.contains("ssg")
            || desc.contains("static")
            || desc.contains("startup")
        {
            matched.push(("ssg", "static site generation keywords"));
        }
        if desc.contains("sse")
            || desc.contains("server-sent")
            || desc.contains("real-time")
            || desc.contains("live")
            || desc.contains("stream")
        {
            matched.push(("sse", "real-time / streaming keywords"));
        }
        if desc.contains("websocket") || desc.contains("ws") || desc.contains("socket") {
            matched.push(("websockets", "WebSocket keywords"));
        }
        if desc.contains("defer") || desc.contains("lazy") || desc.contains("after shell") {
            matched.push(("deferred-streams", "deferred / lazy loading keywords"));
        }
        if desc.contains("action")
            || desc.contains("form")
            || desc.contains("submit")
            || desc.contains("post")
            || desc.contains("creat")
            || desc.contains("updat")
            || desc.contains("delet")
        {
            matched.push(("named-actions", "form / action keywords"));
        }
        if desc.contains("react") || desc.contains("jsx") || desc.contains("tsx") {
            matched.push(("react-islands", "React island / JSX keywords"));
        }
        if desc.contains("layout") || desc.contains("shared") || desc.contains("wrap") {
            matched.push(("layouts", "layout keywords"));
        }
        if desc.contains("error") || desc.contains("not found") || desc.contains("404") {
            matched.push(("error-pages", "error handling keywords"));
        }
        if desc.contains("fragment") || desc.contains("partial") || desc.contains("widget") {
            matched.push(("fragments", "fragment / partial keywords"));
        }
        if desc.contains("middleware")
            || desc.contains("auth")
            || desc.contains("session")
            || desc.contains("guard")
        {
            matched.push(("middleware", "middleware / auth keywords"));
        }
        if (desc.contains("hook") && !desc.contains("react"))
            || desc.contains("global request")
            || desc.contains("before route")
            || desc.contains("startup init")
        {
            matched.push(("server-hooks", "hook lifecycle keywords"));
        }
        if desc.contains("redirect") || desc.contains("navigate") || desc.contains("route") {
            matched.push(("typed-routes", "navigation / routing keywords"));
        }
        if desc.contains("env")
            || desc.contains("config")
            || desc.contains("secret")
            || desc.contains("database_url")
        {
            matched.push(("env-config", "environment / config keywords"));
        }
        if desc.contains("csrf") || desc.contains("forgery") || desc.contains("origin check") {
            matched.push(("csrf", "csrf / request safety keywords"));
        }
        if desc.contains("api")
            || desc.contains("json")
            || desc.contains("rest")
            || desc.contains("endpoint")
        {
            matched.push(("api-routes", "API / JSON endpoint keywords"));
        }
        if desc.contains("load") || desc.contains("page") || desc.contains("props") {
            if !matched.iter().any(|(id, _)| *id == "ssr-pages") {
                matched.push(("ssr-pages", "page / load keywords"));
            }
        }

        // If no features matched, default to ssr-pages as a safe starting point.
        if matched.is_empty() {
            matched.push((
                "ssr-pages",
                "default starting point — refine your description for more specific patterns",
            ));
        }

        // Collect specs for matched features.
        let features: Vec<Value> = matched
            .iter()
            .filter_map(|(id, reason)| {
                self.registry.feature(id).map(|f| {
                    json!({
                        "feature_id": id,
                        "matched_because": reason,
                        "summary": f.summary,
                        "canonical_usage": f.canonical_usage,
                        "constraints": f.constraints,
                        "invalid_examples": f.invalid_examples,
                    })
                })
            })
            .collect();

        let skeleton = if include_code {
            Some(build_code_skeleton(&desc, &matched))
        } else {
            None
        };

        Ok(structured(json!({
            "description": args.description,
            "matched_features": features,
            "code_skeleton": skeleton,
            "next_steps": [
                "Copy the code_skeleton into your pages/ or api/ file.",
                "Run validate_implementation on the file before running cargo build.",
                "Use get_feature_spec for the full spec of any matched feature.",
            ]
        })))
    }

    #[tool(
        description = "Apply a safe, conservative automatic fix for a diagnostic finding. Dry-run by default. Only applies to findings with safe_to_auto_apply=true (e.g. adding async to load())."
    )]
    async fn apply_safe_fix(
        &self,
        Parameters(args): Parameters<ApplySafeFixArgs>,
    ) -> Result<CallToolResult, McpError> {
        let findings: Vec<diagnostics::DiagnosticFinding> =
            match serde_json::from_str(&args.findings_json) {
                Ok(f) => f,
                Err(e) => return Ok(tool_error(format!("failed to parse findings_json: {e}"))),
            };
        let finding = match findings.iter().find(|f| f.finding_id == args.finding_id) {
            Some(f) => f,
            None => {
                return Ok(tool_error(format!(
                    "finding_id '{}' not found in provided findings",
                    args.finding_id
                )))
            }
        };
        let dry_run = args.dry_run.unwrap_or(true);
        Ok(structured(diagnostics::apply_safe_fix(finding, dry_run)))
    }
}

// ── MCP protocol handler ───────────────────────────────────────────────────

#[tool_handler]
impl ServerHandler for PilcrowServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.protocol_version = ProtocolVersion::LATEST;
        info.capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .enable_prompts()
            .build();
        info.server_info = Implementation::from_build_env();
        info.instructions = Some(
            "Pilcrow AI-native MCP server. Use registry tools for feature status, scan_project_context before editing, validate_implementation before scaffolding planned syntax, and orchestrate_feature for safe writes. Use diagnose_project or diagnose_route when there is a build or runtime problem. Use compare_patterns when choosing between implementation approaches.".to_string(),
        );
        info
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let mut resources = docs::resources()
            .into_iter()
            .map(|resource| {
                RawResource::new(resource.uri, resource.title)
                    .with_title(resource.title)
                    .with_description(resource.description)
                    .with_mime_type("application/json")
                    .no_annotation()
            })
            .collect::<Vec<_>>();
        resources.push(
            RawResource::new(CURRENT_PROJECT_URI, "current-project")
                .with_title("Pilcrow current project")
                .with_description(
                    "Structured scan of the current Pilcrow app including semantic route graph and OUT_DIR status.",
                )
                .with_mime_type("application/json")
                .no_annotation(),
        );
        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let text = if request.uri == CURRENT_PROJECT_URI {
            let context = scan_project(&self.project_root, None, None)
                .map_err(|error| McpError::internal_error(error.to_string(), None))?;
            serde_json::to_string_pretty(&context)
        } else if let Some(payload) = self
            .knowledge
            .resource_payload(&request.uri, &self.registry)
        {
            serde_json::to_string_pretty(&payload)
        } else {
            return Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": request.uri })),
            ));
        }
        .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            text,
            request.uri,
        )
        .with_mime_type("application/json")]))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult {
            prompts: vec![
                Prompt::new(
                    "pilcrow-code-review",
                    Some("Review Pilcrow code for convention compliance, planned-feature usage, Silcrow boundary correctness, and production readiness."),
                    Some(vec![
                        PromptArgument::new("code").with_description("The Rust or HTML code to review").with_required(true),
                        PromptArgument::new("path").with_description("File path (optional, improves accuracy)").with_required(false),
                    ]),
                ),
                Prompt::new(
                    "pilcrow-scaffold",
                    Some("Scaffold a new Pilcrow pattern (route, API, middleware, etc.) with dry-run preview and validation."),
                    Some(vec![
                        PromptArgument::new("kind").with_description("Scaffold kind: route, static-page, loaded-page, action-page, deferred-page, api-route, middleware, etc.").with_required(true),
                        PromptArgument::new("name").with_description("Name for the scaffolded item").with_required(true),
                    ]),
                ),
                Prompt::new(
                    "pilcrow-build-diagnosis",
                    Some("Diagnose a Pilcrow build failure from an error log or by running a fresh build."),
                    Some(vec![
                        PromptArgument::new("error_log").with_description("Paste the cargo error output here (optional — tool will build if omitted)").with_required(false),
                    ]),
                ),
                Prompt::new(
                    "pilcrow-feature-explanation",
                    Some("Get a thorough explanation of a Pilcrow feature with local evidence from docs, tests, and examples."),
                    Some(vec![
                        PromptArgument::new("feature").with_description("Feature ID or name (e.g. actions, deferred-streams, fragments)").with_required(true),
                    ]),
                ),
            ],
            next_cursor: None,
            meta: None,
        })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        let args = request.arguments.unwrap_or_default();
        let get = |key: &str| -> String {
            args.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let messages = match request.name.as_str() {
            "pilcrow-code-review" => {
                let code = get("code");
                let path = get("path");
                let path_note = if path.is_empty() {
                    String::new()
                } else {
                    format!(" (file: {path})")
                };
                vec![PromptMessage::new_text(PromptMessageRole::User, format!(
                    "Please review this Pilcrow code{path_note} for:\n\
                    1. Convention compliance (load signature, action return types, Props struct)\n\
                    2. Planned feature usage (Islands, SSG, ISR — must be rejected)\n\
                    3. Silcrow boundary correctness (directives in HTML only; delegate client-runtime to silcrow-mcp)\n\
                    4. Production readiness (error handling, deferred loading, missing skeletons)\n\n\
                    Use validate_implementation, get_feature_spec, and diagnose_route as needed.\n\n\
                    Code:\n```\n{code}\n```"
                ))]
            }
            "pilcrow-scaffold" => {
                let kind = get("kind");
                let name = get("name");
                vec![PromptMessage::new_text(PromptMessageRole::User, format!(
                    "Scaffold a Pilcrow '{kind}' named '{name}'.\n\
                    Steps:\n\
                    1. Use get_feature_spec to check the feature status and constraints.\n\
                    2. Use scan_project_context to check for existing files and Pilcrow.toml config.\n\
                    3. Use orchestrate_feature with dry_run=true first to preview files.\n\
                    4. Present the preview. If it looks correct, re-run with dry_run=false.\n\
                    5. After writing, use validate_implementation on each generated file.\n\
                    Report any validation findings and the final file paths."
                ))]
            }
            "pilcrow-build-diagnosis" => {
                let error_log = get("error_log");
                let log_section = if error_log.is_empty() {
                    "Run why_build_failed without an error_log to trigger a fresh build."
                        .to_string()
                } else {
                    format!("Error log:\n```\n{error_log}\n```")
                };
                vec![PromptMessage::new_text(
                    PromptMessageRole::User,
                    format!(
                        "Diagnose this Pilcrow build failure.\n\
                    Steps:\n\
                    1. Use why_build_failed to categorize the error.\n\
                    2. Use diagnose_codegen to check OUT_DIR and generated file status.\n\
                    3. For route errors, use diagnose_route to inspect the failing route.\n\
                    4. Use propose_fix for each finding.\n\
                    5. Apply safe automatic fixes with apply_safe_fix (dry_run=true first).\n\n\
                    {log_section}"
                    ),
                )]
            }
            "pilcrow-feature-explanation" => {
                let feature = get("feature");
                vec![PromptMessage::new_text(PromptMessageRole::User, format!(
                    "Explain the Pilcrow '{feature}' feature thoroughly.\n\
                    Steps:\n\
                    1. Use get_feature_spec for the canonical spec, constraints, and invalid examples.\n\
                    2. Use explain_feature for local docs/tests/examples evidence.\n\
                    3. Use find_examples to find sandbox patterns.\n\
                    4. If the feature touches Silcrow, note the boundary and what needs silcrow-mcp.\n\
                    Synthesize a complete explanation from the evidence."
                ))]
            }
            other => {
                return Err(McpError::invalid_request(
                    format!("unknown prompt: {other}"),
                    None,
                ))
            }
        };

        Ok(GetPromptResult::new(messages))
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn structured(value: impl serde::Serialize) -> CallToolResult {
    match serde_json::to_value(value) {
        Ok(value) => CallToolResult::structured(value),
        Err(error) => tool_error(error.to_string()),
    }
}

fn tool_error(message: impl Into<String>) -> CallToolResult {
    CallToolResult::structured_error(json!({ "error": message.into() }))
}

fn parse_status(value: &str) -> std::result::Result<FeatureStatus, String> {
    match value {
        "stable" => Ok(FeatureStatus::Stable),
        "experimental" => Ok(FeatureStatus::Experimental),
        "planned" => Ok(FeatureStatus::Planned),
        "deprecated" => Ok(FeatureStatus::Deprecated),
        other => Err(format!("unknown feature status: {other}")),
    }
}

fn parse_domain(value: &str) -> std::result::Result<FeatureDomain, String> {
    match value {
        "pilcrow" => Ok(FeatureDomain::Pilcrow),
        "silcrow" => Ok(FeatureDomain::Silcrow),
        other => Err(format!("unknown feature domain: {other}")),
    }
}

fn suggest_from_context(
    context: &crate::workspace::ProjectContext,
    focus: Option<String>,
) -> Optimizations {
    let mut recommendations = Vec::new();
    if context.loading_skeletons.is_empty() {
        recommendations.push(Optimization {
            rule_id: "pilcrow-loading-skeletons",
            severity: "info",
            message: "No _loading.html templates were found.".to_string(),
            suggested_fix: "Add scoped _loading.html templates near routes that fetch remote data or use deferred streams.".to_string(),
        });
    }
    if context.middleware.is_none() {
        recommendations.push(Optimization {
            rule_id: "pilcrow-middleware",
            severity: "info",
            message: "No hooks.rs was found.".to_string(),
            suggested_fix: "Add middleware only when cross-cutting auth, headers, tracing, or request locals are needed.".to_string(),
        });
    }
    if context.fragments.is_empty() {
        recommendations.push(Optimization {
            rule_id: "pilcrow-fragments",
            severity: "warning",
            message: "No fragment groups are configured in Pilcrow.toml.".to_string(),
            suggested_fix: "Configure [[fragments]] when repeated UI needs addressable server-rendered partials.".to_string(),
        });
    }
    if context.generated_out_dir.path.is_none() {
        recommendations.push(Optimization {
            rule_id: "pilcrow-codegen-status",
            severity: "warning",
            message: context.generated_out_dir.message.clone(),
            suggested_fix: "Run codegen_build before inspecting generated routes or typed helpers."
                .to_string(),
        });
    }
    // Check for deferred routes missing loading skeletons
    let deferred_routes: Vec<_> = context
        .route_graph
        .iter()
        .filter(|n| {
            n.code_behind
                .as_ref()
                .map(|cb| cb.has_deferred)
                .unwrap_or(false)
                && !n.has_loading
        })
        .collect();
    if !deferred_routes.is_empty() {
        recommendations.push(Optimization {
            rule_id: "pilcrow-deferred-needs-skeleton",
            severity: "warning",
            message: format!(
                "{} route(s) use AsyncValue<T> or AsyncHtml but have no _loading.html skeleton.",
                deferred_routes.len()
            ),
            suggested_fix: "Add _loading.html templates near deferred routes for better UX."
                .to_string(),
        });
    }
    if matches!(focus.as_deref(), Some("silcrow")) {
        recommendations.push(Optimization {
            rule_id: "silcrow-server-backed-interactions",
            severity: "info",
            message: "Silcrow interactions should preserve plain HTML behavior and use server actions for state.".to_string(),
            suggested_fix: "Prefer forms posting to ?/action and anchors with s-boost/s-target over client-only state.".to_string(),
        });
    }
    Optimizations {
        focus,
        recommendations,
    }
}

/// Generate a minimal but complete code skeleton based on the detected feature set.
fn build_code_skeleton(desc: &str, matched: &[(&str, &str)]) -> Value {
    let ids: Vec<&str> = matched.iter().map(|(id, _)| *id).collect();

    let has = |id: &str| ids.contains(&id);

    // Determine the primary pattern and produce the appropriate skeleton.
    if has("react-islands") {
        json!({
            "toml": "// Pilcrow.toml\n[routing]\nignore_directories = [\"react\"]\n\n[client.react]\nenabled = true\ndirs = [\"react\"]",
            "html": "<!-- pages/products/index.html -->\n<h1>{{ title }}</h1>\n<react src=\"./react/ProductPanel.jsx\" strategy=\"visible\" path=\"/products\" />",
            "jsx": "import { Suspense, use, useMemo } from \"react\";\nimport { useSilcrowAtom, useSilcrowAction } from \"pilcrow/react\";\n\nfunction ProductRows({ promise }) {\n  const initial = use(promise);\n  const data = useSilcrowAtom(\"route:/products\", initial);\n  return <ul>{data.items.map((item) => <li key={item.id}>{item.name}</li>)}</ul>;\n}\n\nexport default function ProductPanel({ path = \"/products\" }) {\n  const promise = useMemo(() => window.Silcrow.prefetch(path), [path]);\n  const [state, createProduct, pending] = useSilcrowAction(\n    \"?/create\",\n    (result, prev) => result.data ?? prev,\n    { ok: true },\n    { scope: \"products:create\" },\n  );\n\n  return (\n    <section>\n      <Suspense fallback={<p>Loading products...</p>}>\n        <ProductRows promise={promise} />\n      </Suspense>\n      <form action={createProduct}>\n        <input name=\"name\" />\n        <button disabled={pending}>Add</button>\n        {state.message ? <p>{state.message}</p> : null}\n      </form>\n    </section>\n  );\n}",
            "rs": "// pages/products/index.rs\nuse pilcrow_web::{form_errors, json, ActionResult, AppResult, Req};\n\npub struct Props { pub title: String }\n\npub async fn load(_req: Req) -> AppResult<Props> {\n    Ok(Props { title: \"Products\".into() })\n}\n\npub async fn create(req: Req) -> ActionResult {\n    let name = req.form.get(\"name\").unwrap_or(\"\").trim();\n    if name.is_empty() {\n        return req.fail(form_errors().error(\"name\", \"Name is required\"));\n    }\n    req.res.invalidate_target(\"#products\");\n    json(serde_json::json!({ \"ok\": true, \"message\": \"Product created\" }))\n}",
            "note": "React sources can be .jsx. TypeScript is optional. Server loading/actions stay in the containing page or fragment code-behind; React imports hooks from the generated pilcrow/react Vite alias.",
        })
    } else if has("incremental-ssr") && has("ssg") {
        // Combined PRERENDER + REVALIDATE
        json!({
            "rs": "// pages/products/index.rs\npub const PRERENDER: bool = true;\npub const REVALIDATE: u64 = 60;\npub const CACHE_TAGS: &[&str] = &[\"products\"];\n\npub struct Props { pub items: Vec<String> }\n\npub async fn load(_req: Req) -> AppResult<Props> {\n    Ok(Props { items: vec![] })\n}",
            "html": "<!-- pages/products/index.html -->\n{% for item in items %}<li>{{ item }}</li>{% endfor %}",
            "note": "Prerendered at startup, then ISR-revalidated every 60s. Use pilcrow_start() in main.rs.",
        })
    } else if has("incremental-ssr") {
        let ttl = if desc.contains("60") {
            60
        } else if desc.contains("300") {
            300
        } else {
            60
        };
        json!({
            "rs": format!("// pages/products/index.rs\npub const REVALIDATE: u64 = {ttl};\npub const CACHE_TAGS: &[&str] = &[\"products\"];\n\npub struct Props {{ pub items: Vec<String> }}\n\npub async fn load(_req: Req) -> AppResult<Props> {{\n    Ok(Props {{ items: vec![] }})\n}}"),
            "html": "<!-- pages/products/index.html -->\n{% for item in items %}<li>{{ item }}</li>{% endfor %}",
            "invalidation": "Call req.cache.revalidate_tag(\"products\") in an action to bust the cache.",
        })
    } else if has("ssg") {
        json!({
            "rs": "// pages/about.rs\npub const PRERENDER: bool = true;\n\npub struct Props { pub title: String }\n\npub async fn load(_req: Req) -> AppResult<Props> {\n    Ok(Props { title: \"About\".into() })\n}",
            "html": "<!-- pages/about.html -->\n<h1>{{ title }}</h1>",
            "note": "Rendered once at startup. Use pilcrow_start() in main.rs instead of pilcrow_web::start().",
        })
    } else if has("named-actions") && has("ssr-pages") {
        json!({
            "rs": "// pages/items.rs\npub struct Props { pub items: Vec<String> }\n\npub async fn load(_req: Req) -> AppResult<Props> {\n    Ok(Props { items: vec![] })\n}\n\npub async fn create(req: Req) -> ActionResult {\n    let name = req.form.get(\"name\").unwrap_or(\"\");\n    if name.is_empty() {\n        return req.fail(form_errors().error(\"name\", \"required\").value(\"name\", name));\n    }\n    redirect(\"/items\")\n}",
            "html": "<!-- pages/items.html -->\n<form s-post=\"?/create\" s-target=\"#form\">\n  <input name=\"name\" />\n  <span :text=\"errors.name\"></span>\n  <button>Add</button>\n</form>",
        })
    } else if has("named-actions") {
        json!({
            "rs": "// pages/items.rs\npub async fn create(req: Req) -> ActionResult {\n    let name = req.form.get(\"name\").unwrap_or(\"\");\n    redirect(\"/items\")\n}\n\npub async fn delete(req: Req) -> ActionResult {\n    let id = req.params.get(\"id\").map(|s| s.as_str()).unwrap_or(\"\");\n    redirect(\"/items\")\n}",
            "html": "<!-- pages/items.html -->\n<form s-post=\"?/create\">...</form>\n<button s-post=\"?/delete\">Delete</button>",
        })
    } else if has("sse") {
        json!({
            "rs": "// api/counter.rs\npub async fn get(req: Req) -> Response {\n    pilcrow_web::sse(async_stream::stream! {\n        let mut n = 0u64;\n        loop {\n            yield pilcrow_web::SseEvent::json(serde_json::json!({ \"count\": n }));\n            n += 1;\n            tokio::time::sleep(std::time::Duration::from_secs(1)).await;\n        }\n    })\n}",
            "html": "<!-- pages/index.html -->\n<span :text=\"count\" s-sse=\"/counter\">0</span>",
        })
    } else if has("deferred-streams") {
        json!({
            "rs": "// pages/dashboard.rs\nuse pilcrow_web::AsyncValue;\n\npub struct Props {\n    pub title: String,\n    pub count: AsyncValue<i64>,\n}\n\npub async fn load(_req: Req) -> AppResult<Props> {\n    Ok(Props {\n        title: \"Dashboard\".into(),\n        count: AsyncValue::spawn(async { expensive_db_count().await }),\n    })\n}",
            "html": "<!-- pages/dashboard.html -->\n<h1>{{ title }}</h1>\n<span :text=\"count\">…</span>",
        })
    } else if has("api-routes") {
        json!({
            "rs": "// api/products.rs\nuse pilcrow_web::{Req, json};\nuse axum::response::Response;\n\npub async fn get(_req: Req) -> Response {\n    json(serde_json::json!({ \"products\": [] }))\n}",
            "note": "File becomes GET /api/products. Add post(), put(), delete() for other methods.",
        })
    } else if has("middleware") {
        json!({
            "rs": "// hooks.rs\nuse pilcrow_web::{AppError, Next, Req, Response};\nuse axum::response::IntoResponse;\n\npub async fn middleware(req: Req, next: Next) -> Response {\n    let token = req.cookies.get(\"session\").map(|c| c.value().to_string());\n    match verify_session(token).await {\n        Ok(user) => req.locals.set(user),\n        Err(_) if req.path.starts_with(\"/admin\") => {\n            return AppError::Unauthorized.into_response();\n        }\n        _ => {}\n    }\n    next.run().await\n}",
            "note": "Detected automatically — place at hooks.rs. No registration needed.",
        })
    } else {
        // Default: basic loaded page
        json!({
            "rs": "// pages/index.rs\npub struct Props { pub title: String }\n\npub async fn load(_req: Req) -> AppResult<Props> {\n    Ok(Props { title: \"Hello, Pilcrow\".into() })\n}",
            "html": "<!-- pages/index.html -->\n<h1>{{ title }}</h1>",
        })
    }
}

fn compare_patterns_for(goal: &str, _options: Option<&Value>) -> PatternComparison {
    let goal_lower = goal.to_ascii_lowercase();

    let patterns = if goal_lower.contains("form")
        || goal_lower.contains("action")
        || goal_lower.contains("submit")
    {
        vec![
            Pattern {
                name: "Silcrow enhanced form".to_string(),
                description: "Form posts to ?/action, server patches DOM via s-target. Progressive enhancement — works without JS.".to_string(),
                tradeoffs: vec![
                    "Pro: works without JS, server owns state".to_string(),
                    "Pro: req.fail() handles both enhanced and plain POST".to_string(),
                    "Con: requires silcrow.js loaded on client".to_string(),
                ],
                scaffold_kind: Some("silcrow-form".to_string()),
                status: "stable".to_string(),
            },
            Pattern {
                name: "Plain action page".to_string(),
                description: "Standard HTML form POST to ?/action, server redirects after success (PRG pattern).".to_string(),
                tradeoffs: vec![
                    "Pro: zero JS dependency".to_string(),
                    "Pro: full page reloads ensure freshness".to_string(),
                    "Con: no in-place DOM patching".to_string(),
                ],
                scaffold_kind: Some("action-page".to_string()),
                status: "stable".to_string(),
            },
        ]
    } else if goal_lower.contains("stream")
        || goal_lower.contains("defer")
        || goal_lower.contains("lazy")
    {
        vec![
            Pattern {
                name: "AsyncValue<T>".to_string(),
                description: "Stream a single typed value after the shell renders. Use for data that is slow to fetch but simple to display.".to_string(),
                tradeoffs: vec![
                    "Pro: shell renders immediately".to_string(),
                    "Pro: silcrow.js patches the value in-place".to_string(),
                    "Con: T must implement Display for the initial empty render".to_string(),
                ],
                scaffold_kind: Some("deferred-page".to_string()),
                status: "stable".to_string(),
            },
            Pattern {
                name: "AsyncHtml".to_string(),
                description: "Stream a complete HTML fragment into a named slot. Use for complex widgets or lists that render as HTML.".to_string(),
                tradeoffs: vec![
                    "Pro: can stream arbitrary HTML markup".to_string(),
                    "Pro: loading HTML shown in slot until resolved".to_string(),
                    "Con: more verbose setup than AsyncValue<T>".to_string(),
                ],
                scaffold_kind: None,
                status: "stable".to_string(),
            },
        ]
    } else if goal_lower.contains("layout")
        || goal_lower.contains("shell")
        || goal_lower.contains("nav")
    {
        vec![
            Pattern {
                name: "Root _layout.html".to_string(),
                description: "Single layout wrapping all pages at pages/_layout.html.".to_string(),
                tradeoffs: vec![
                    "Pro: simple, applies everywhere".to_string(),
                    "Con: cannot be scoped to a subset of routes".to_string(),
                ],
                scaffold_kind: Some("nested-layout".to_string()),
                status: "stable".to_string(),
            },
            Pattern {
                name: "Route group layout".to_string(),
                description: "Scoped layout in (group)/_layout.html applies only to sibling routes without adding a URL segment.".to_string(),
                tradeoffs: vec![
                    "Pro: scoped to a logical section without URL impact".to_string(),
                    "Con: requires route group directory organization".to_string(),
                ],
                scaffold_kind: Some("nested-layout".to_string()),
                status: "stable".to_string(),
            },
        ]
    } else if goal_lower.contains("api")
        || goal_lower.contains("endpoint")
        || goal_lower.contains("rest")
    {
        vec![
            Pattern {
                name: "API route file".to_string(),
                description: "File in api/ exports router(). Discovered automatically by routekit.".to_string(),
                tradeoffs: vec![
                    "Pro: auto-mounted, no main.rs changes".to_string(),
                    "Pro: full axum Router flexibility".to_string(),
                    "Con: separate from page route structure".to_string(),
                ],
                scaffold_kind: Some("api-route".to_string()),
                status: "stable".to_string(),
            },
            Pattern {
                name: "Named action on page".to_string(),
                description: "pub async fn action_name(req: Req) -> ActionResult on a page or fragment code-behind. Invoked by POST ?/action_name on that route.".to_string(),
                tradeoffs: vec![
                    "Pro: co-located with the page that uses it".to_string(),
                    "Pro: shares the page's load() and layout context".to_string(),
                    "Con: POST-only, not a standalone REST endpoint".to_string(),
                ],
                scaffold_kind: Some("action-page".to_string()),
                status: "stable".to_string(),
            },
        ]
    } else {
        vec![Pattern {
            name: "SSR page with load()".to_string(),
            description: "Standard Pilcrow page with Props and load() for server data.".to_string(),
            tradeoffs: vec![
                "Pro: full server control".to_string(),
                "Pro: no client state".to_string(),
            ],
            scaffold_kind: Some("loaded-page".to_string()),
            status: "stable".to_string(),
        }]
    };

    let recommendation = patterns
        .first()
        .map(|p| format!("For '{}', start with '{}'. {}", goal, p.name, p.tradeoffs.first().cloned().unwrap_or_default()))
        .unwrap_or_else(|| format!("No specific pattern matched for '{}'. Use scan_project_context and answer_pilcrow_question for guidance.", goal));

    PatternComparison {
        goal: goal.to_string(),
        patterns,
        recommendation,
    }
}

fn analyse_build_error(error_log: &str) -> Value {
    let mut categories = Vec::new();
    let mut suggestions = Vec::new();

    if error_log.contains("load")
        && (error_log.contains("async") || error_log.contains("not async"))
    {
        categories.push("load() signature mismatch");
        suggestions.push("Ensure load() is `pub async fn load(req: Req) -> AppResult<Props>` or, on dynamic pages, `pub async fn load(ctx: Page) -> AppResult<Props>`. Missing async is the most common cause.");
    }
    if error_log.contains("Props") && error_log.contains("field") {
        categories.push("Props field mismatch");
        suggestions.push("Check that Props fields in the .rs file match template variable usage. Layouts and pages cannot have colliding field names.");
    }
    if error_log.contains("ActionResult") {
        categories.push("ActionResult type error");
        suggestions.push(
            "Named action functions must return ActionResult. Check imports and return type.",
        );
    }
    if error_log.contains("OUT_DIR")
        || error_log.contains("generated_app")
        || error_log.contains("include!")
    {
        categories.push("Generated code error");
        suggestions.push("Run codegen_build to regenerate OUT_DIR. Check routekit pipeline output for template or code-behind errors.");
    }
    if error_log.contains("cannot find type `Req`") || error_log.contains("Req` is not in scope") {
        categories.push("Missing import for Req");
        suggestions.push("The framework injects `use pilcrow_web::Req;` automatically in generated code. If writing standalone code, add the import manually.");
    }
    if error_log.contains("cannot find function `redirect`") {
        categories.push("Missing redirect import");
        suggestions.push("Add `use pilcrow_web::redirect;` or rely on the auto-injected import in code-behind files.");
    }
    if error_log.contains("field collision") || error_log.contains("defined in both") {
        categories.push("Layout/page Props field collision");
        suggestions.push("A field name is defined in both the layout's Props and the page's Props. Rename one to avoid the collision.");
    }
    if error_log.contains("invalid Pilcrow route configuration") {
        categories.push("Invalid route configuration");
        suggestions.push("Read the route/module and suggested fix in the build error. Common causes include STREAMING combined with REVALIDATE, STREAMING combined with PRERENDER, STREAMING with AsyncValue/AsyncHtml fields, or dynamic PRERENDER without entries().");
    }

    if categories.is_empty() {
        categories.push("unrecognized error");
        suggestions.push("Use diagnose_project to scan for structural issues. Check the full cargo error output for the root cause.");
    }

    json!({
        "categories": categories,
        "suggestions": suggestions,
        "raw_excerpt": error_log.lines().filter(|l| l.contains("error") || l.contains("error[")).take(10).collect::<Vec<_>>(),
    })
}
