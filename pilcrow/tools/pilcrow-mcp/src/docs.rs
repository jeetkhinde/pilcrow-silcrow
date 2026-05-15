use crate::registry::{Feature, FeatureDomain, Registry};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{fs, path::Path};

pub const DOCS_URI: &str = "pilcrow://docs";
pub const API_RUNTIME_URI: &str = "pilcrow://api/runtime";
pub const API_WEB_URI: &str = "pilcrow://api/web";
pub const ROUTEKIT_FEATURES_URI: &str = "pilcrow://routekit/features";
pub const EXAMPLES_URI: &str = "pilcrow://examples";
pub const TEST_MATRIX_URI: &str = "pilcrow://tests/feature-matrix";

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeResource {
    pub uri: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub category: KnowledgeCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnowledgeCategory {
    Docs,
    RuntimeApi,
    WebApi,
    Routekit,
    Examples,
    Tests,
}

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeDocument {
    pub id: String,
    pub title: String,
    pub path: String,
    pub category: KnowledgeCategory,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    pub title: String,
    pub path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourcePayload {
    pub uri: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub documents: Vec<KnowledgeDocument>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureExplanation {
    pub feature: Feature,
    pub implementation_status: &'static str,
    pub silcrow_boundary: Option<String>,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExampleSearch {
    pub query: String,
    pub examples: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpertAnswer {
    pub question: String,
    pub answer: String,
    pub status_note: String,
    pub silcrow_delegation: Option<String>,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone)]
pub struct KnowledgeBase {
    project_root: std::path::PathBuf,
    documents: Vec<KnowledgeDocument>,
}

pub fn resources() -> Vec<KnowledgeResource> {
    vec![
        KnowledgeResource {
            uri: DOCS_URI,
            title: "Pilcrow docs",
            description: "Project-level Pilcrow guidance from CLAUDE.md and routekit README.",
            category: KnowledgeCategory::Docs,
        },
        KnowledgeResource {
            uri: API_RUNTIME_URI,
            title: "Pilcrow runtime API",
            description: "Runtime request, response, middleware, deferred, SSE, and WebSocket APIs.",
            category: KnowledgeCategory::RuntimeApi,
        },
        KnowledgeResource {
            uri: API_WEB_URI,
            title: "Pilcrow web facade API",
            description: "pilcrow-web exports and generated app integration.",
            category: KnowledgeCategory::WebApi,
        },
        KnowledgeResource {
            uri: ROUTEKIT_FEATURES_URI,
            title: "Routekit features",
            description: "Routekit routing, templating, codegen, page options, and feature registry.",
            category: KnowledgeCategory::Routekit,
        },
        KnowledgeResource {
            uri: EXAMPLES_URI,
            title: "Pilcrow examples",
            description: "Sandbox app examples for pages, layouts, actions, fragments, APIs, and UI templates.",
            category: KnowledgeCategory::Examples,
        },
        KnowledgeResource {
            uri: TEST_MATRIX_URI,
            title: "Pilcrow test feature matrix",
            description: "Routekit and runtime tests grouped as evidence for implemented behavior.",
            category: KnowledgeCategory::Tests,
        },
    ]
}

impl KnowledgeBase {
    pub fn load(project_root: &Path) -> Result<Self> {
        let specs = document_specs();
        let mut documents = Vec::new();
        for spec in specs {
            let path = project_root.join(spec.path);
            if !path.exists() {
                continue;
            }
            let text = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            documents.push(KnowledgeDocument {
                id: spec.id.to_string(),
                title: spec.title.to_string(),
                path: spec.path.to_string(),
                category: spec.category,
                text,
            });
        }
        Ok(Self {
            project_root: project_root.to_path_buf(),
            documents,
        })
    }

    pub fn resource_payload(&self, uri: &str, registry: &Registry) -> Option<ResourcePayload> {
        let resource = resources()
            .into_iter()
            .find(|resource| resource.uri == uri)?;
        let mut documents = self
            .documents
            .iter()
            .filter(|document| document.category == resource.category)
            .cloned()
            .collect::<Vec<_>>();
        if uri == ROUTEKIT_FEATURES_URI {
            documents.push(KnowledgeDocument {
                id: "feature-registry".to_string(),
                title: "Pilcrow feature registry".to_string(),
                path: "registry.toml".to_string(),
                category: KnowledgeCategory::Routekit,
                text: serde_json::to_string_pretty(&registry.features).unwrap_or_default(),
            });
        }
        Some(ResourcePayload {
            uri: resource.uri,
            title: resource.title,
            description: resource.description,
            documents,
        })
    }

    pub fn explain_feature(&self, registry: &Registry, id: &str) -> Option<FeatureExplanation> {
        let feature = registry.feature(id)?.clone();
        let mut query = format!("{} {} {}", feature.id, feature.name, feature.summary);
        query.push(' ');
        query.push_str(&feature.validation_rules.join(" "));
        let evidence = self.search(&query, 6);
        let silcrow_boundary = match feature.domain {
            FeatureDomain::Silcrow => Some(
                "Pilcrow MCP only covers server-side integration. Use silcrow-mcp for exact silcrow.js runtime behavior.".to_string(),
            ),
            FeatureDomain::Pilcrow if mentions_silcrow(&query) => Some(
                "This Pilcrow feature touches Silcrow integration; defer exact client runtime semantics to silcrow-mcp.".to_string(),
            ),
            FeatureDomain::Pilcrow => None,
        };
        Some(FeatureExplanation {
            implementation_status: status_label(&feature.status),
            feature,
            silcrow_boundary,
            evidence,
        })
    }

    pub fn find_examples(
        &self,
        feature: &str,
        pattern: Option<&str>,
        limit: usize,
    ) -> ExampleSearch {
        let query = [feature, pattern.unwrap_or("")]
            .into_iter()
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let examples = self
            .documents
            .iter()
            .filter(|document| {
                matches!(
                    document.category,
                    KnowledgeCategory::Examples | KnowledgeCategory::Tests
                )
            })
            .flat_map(|document| document_hits(document, &terms(&query), 3))
            .take(limit)
            .collect();
        ExampleSearch { query, examples }
    }

    pub fn answer_question(&self, registry: &Registry, question: &str) -> ExpertAnswer {
        let evidence = self.search(question, 8);
        let features = matching_features(registry, question);
        let status_note = if features.is_empty() {
            "No registry feature matched directly; answer is based on local docs/tests/examples."
                .to_string()
        } else {
            features
                .iter()
                .map(|feature| format!("{} is {}.", feature.id, status_label(&feature.status)))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let silcrow_delegation = if needs_silcrow_delegation(question) {
            Some("This question asks about exact silcrow.js client-runtime behavior. Pilcrow MCP can validate the server-side Pilcrow shape, but use silcrow-mcp for directive, DOM patching, cache, and navigation semantics.".to_string())
        } else {
            None
        };
        let answer = build_answer(question, &features, &evidence, silcrow_delegation.is_some());
        ExpertAnswer {
            question: question.to_string(),
            answer,
            status_note,
            silcrow_delegation,
            evidence,
        }
    }

    fn search(&self, query: &str, limit: usize) -> Vec<Evidence> {
        let query_terms = terms(query);
        let mut hits = self
            .documents
            .iter()
            .flat_map(|document| document_hits(document, &query_terms, 2))
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            score(&right.excerpt, &query_terms).cmp(&score(&left.excerpt, &query_terms))
        });
        hits.truncate(limit);
        hits
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}

fn document_hits(
    document: &KnowledgeDocument,
    query_terms: &[String],
    context: usize,
) -> Vec<Evidence> {
    if query_terms.is_empty() {
        return vec![];
    }
    let lines = document.text.lines().collect::<Vec<_>>();
    let mut hits = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let lower = line.to_ascii_lowercase();
        if !query_terms.iter().any(|term| lower.contains(term)) {
            continue;
        }
        let start = idx.saturating_sub(context);
        let end = (idx + context + 1).min(lines.len());
        hits.push(Evidence {
            title: document.title.clone(),
            path: document.path.clone(),
            line_start: start + 1,
            line_end: end,
            excerpt: lines[start..end].join("\n"),
        });
        if hits.len() >= 4 {
            break;
        }
    }
    hits
}

fn build_answer(
    question: &str,
    features: &[&Feature],
    evidence: &[Evidence],
    silcrow_delegation: bool,
) -> String {
    if silcrow_delegation {
        return "This is partly outside Pilcrow MCP ownership: exact silcrow.js runtime behavior belongs to silcrow-mcp. On the Pilcrow side, use server-rendered pages, code-behind actions, response helpers, and routekit-generated wiring; validate the server shape here, then ask silcrow-mcp for client directive/runtime details.".to_string();
    }
    if let Some(feature) = features.first() {
        return format!(
            "{} is currently {}. {} Use the evidence list for the local docs/tests/examples that support this.",
            feature.name,
            status_label(&feature.status),
            feature.spec
        );
    }
    if evidence.is_empty() {
        return format!(
            "I could not find local Pilcrow evidence for `{question}`. Treat this as unknown until docs, tests, or implementation references are added."
        );
    }
    "The relevant local evidence is in the returned excerpts. Use those docs/tests/examples as the authority before changing Pilcrow code.".to_string()
}

fn matching_features<'a>(registry: &'a Registry, query: &str) -> Vec<&'a Feature> {
    let query_terms = terms(query);
    registry
        .features
        .iter()
        .filter(|feature| {
            let haystack = format!(
                "{} {} {} {}",
                feature.id, feature.name, feature.summary, feature.spec
            )
            .to_ascii_lowercase();
            query_terms.iter().any(|term| haystack.contains(term))
        })
        .take(5)
        .collect()
}

fn terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        .map(str::trim)
        .filter(|term| term.len() >= 3)
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

fn score(text: &str, query_terms: &[String]) -> usize {
    let lower = text.to_ascii_lowercase();
    query_terms
        .iter()
        .map(|term| lower.matches(term).count())
        .sum()
}

fn mentions_silcrow(text: &str) -> bool {
    text.to_ascii_lowercase().contains("silcrow")
}

fn needs_silcrow_delegation(question: &str) -> bool {
    let lower = question.to_ascii_lowercase();
    lower.contains("silcrow.js")
        || lower.contains("dom patch")
        || lower.contains("client runtime")
        || lower.contains("s-boost")
        || lower.contains("s-target")
        || lower.contains("s-swap")
        || lower.contains("cache behavior")
}

fn status_label(status: &crate::registry::FeatureStatus) -> &'static str {
    match status {
        crate::registry::FeatureStatus::Stable => "stable",
        crate::registry::FeatureStatus::Experimental => "experimental",
        crate::registry::FeatureStatus::Planned => "planned",
        crate::registry::FeatureStatus::Deprecated => "deprecated",
    }
}

struct DocumentSpec {
    id: &'static str,
    title: &'static str,
    path: &'static str,
    category: KnowledgeCategory,
}

fn document_specs() -> Vec<DocumentSpec> {
    vec![
        DocumentSpec {
            id: "claude",
            title: "Project guidance",
            path: "CLAUDE.md",
            category: KnowledgeCategory::Docs,
        },
        DocumentSpec {
            id: "routekit-readme",
            title: "Routekit README",
            path: "crates/routekit/README.md",
            category: KnowledgeCategory::Docs,
        },
        DocumentSpec {
            id: "experimental-baked-pages-guide",
            title: "Experimental baked pages guide",
            path: "docs/experimental-baked-pages.md",
            category: KnowledgeCategory::Docs,
        },
        DocumentSpec {
            id: "web-facade",
            title: "pilcrow-web facade",
            path: "crates/web/src/lib.rs",
            category: KnowledgeCategory::WebApi,
        },
        DocumentSpec {
            id: "runtime-context",
            title: "Runtime request context",
            path: "crates/runtime/src/context.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-response",
            title: "Runtime response helpers",
            path: "crates/runtime/src/response/response.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-deferred",
            title: "AsyncValue/AsyncHtml streaming, LiveProp<T> SSE live props, and SSR Streaming",
            path: "crates/runtime/src/deferred.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "fsr-live-surface",
            title: "FSR developer surface: `use pilcrow::live::*;`",
            path: "crates/web/src/live.rs",
            category: KnowledgeCategory::WebApi,
        },
        DocumentSpec {
            id: "fsr-mod",
            title: "FSR runtime module (LiveProp, LiveQuery, PilcrowLive, watcher, hub)",
            path: "crates/runtime/src/fsr/mod.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "fsr-live-props",
            title: "FSR LiveProp<T> and DependencyKey types",
            path: "crates/runtime/src/fsr/live_props.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "fsr-live-trait",
            title: "FSR PilcrowLive trait and LiveQuery",
            path: "crates/runtime/src/fsr/live_trait.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "fsr-macros",
            title: "FSR macros: fsr_dep!, live_query!",
            path: "crates/runtime/src/fsr/macros.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "fsr-routekit-validation",
            title: "FSR routekit validation for live.rs and s-live slots",
            path: "crates/routekit/src/fsr.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "fsr-cache",
            title: "FSR Redis cache layer (RedisCache, pub/sub payloads)",
            path: "crates/runtime/src/fsr/cache.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "fsr-store",
            title: "FSR Postgres store (FsrStore, StaleSlot, InspectRow)",
            path: "crates/runtime/src/fsr/store.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-middleware",
            title: "Middleware",
            path: "crates/runtime/src/middleware.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-isr",
            title: "ISR cache runtime",
            path: "crates/runtime/src/isr.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-baked-pages",
            title: "Experimental baked-page declaration model",
            path: "crates/runtime/src/baked_pages/model.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-baked-page-store",
            title: "Experimental baked-page artifact store",
            path: "crates/runtime/src/baked_pages/store.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-baked-page-patching",
            title: "Experimental baked-page slot patching",
            path: "crates/runtime/src/baked_pages/patch.rs",
            category: KnowledgeCategory::RuntimeApi,
        },

        DocumentSpec {
            id: "runtime-baked-page-serving",
            title: "Experimental baked-page lazy serving helper",
            path: "crates/runtime/src/baked_pages/serving.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-start",
            title: "Server startup (start / start_with_prerender)",
            path: "crates/runtime/src/start.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-csrf",
            title: "CSRF protection middleware",
            path: "crates/runtime/src/csrf.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-adapter",
            title: "PilcrowAdapter trait and TokioAdapter",
            path: "crates/runtime/src/adapter.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-adapters-server",
            title: "PortEnvAdapter — Fly/Railway/Cloud Run/Render/Vercel adapters",
            path: "crates/runtime/src/adapters/server.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-adapters-lambda",
            title: "LambdaAdapter — AWS Lambda / Vercel Functions / Netlify Functions",
            path: "crates/runtime/src/adapters/lambda.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-dev",
            title: "Dev server: live reload, CSS hot swap, build banner",
            path: "crates/runtime/src/dev.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-sw",
            title: "Service worker: generate_sw_source(), sw_handler(), sw_inject_layer()",
            path: "crates/runtime/src/sw.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-i18n",
            title: "i18n: I18nBundles, locale_middleware_impl, FmtHelper",
            path: "crates/runtime/src/i18n.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "routekit-i18n-codegen",
            title: "i18n codegen: FTL parser → pub mod t typed translation helpers",
            path: "crates/routekit/src/templating/i18n_codegen.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "runtime-image",
            title: "Image optimization: ImageState, image_handler, /_image endpoint",
            path: "crates/runtime/src/image/handler.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-image-processor",
            title: "Image processor: transform, cache_path, OutputFormat, resize",
            path: "crates/runtime/src/image/processor.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "core-config",
            title: "PilcrowConfig, I18nConfig, ImageConfig, ServiceWorkerConfig, SwStrategy, CacheConfig",
            path: "crates/core/src/config/config.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "routekit-lib",
            title: "Routekit library",
            path: "crates/routekit/src/lib.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "routekit-pipeline",
            title: "Routekit compile pipeline",
            path: "crates/routekit/src/templating/pipeline.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "routekit-compiler",
            title: "Template compiler",
            path: "crates/routekit/src/templating/compiler.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "routekit-build-config",
            title: "Pilcrow build config",
            path: "crates/routekit/src/templating/build_config.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "routekit-page-options",
            title: "Page options (TRAILING_SLASH, LAYOUT, REVALIDATE, PRERENDER, STREAMING)",
            path: "crates/routekit/src/templating/page_options.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "routekit-routes-codegen",
            title: "Typed route helper codegen",
            path: "crates/routekit/src/templating/routes_codegen.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "routekit-env-codegen",
            title: "Typed env struct codegen",
            path: "crates/routekit/src/templating/env_codegen.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "routekit-codegen-types",
            title: "Generated template metadata",
            path: "crates/routekit/src/templating/codegen/types.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "routekit-app-codegen",
            title: "Generated app module (ISR, SSG, STREAMING, Deferred handler emitters)",
            path: "crates/routekit/src/templating/codegen/app_module.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "cli-scaffold",
            title: "CLI scaffold (new + --with-auth/--with-postgres)",
            path: "tools/cli/src/scaffold.rs",
            category: KnowledgeCategory::Examples,
        },
        DocumentSpec {
            id: "cli-export",
            title: "CLI export command",
            path: "tools/cli/src/export.rs",
            category: KnowledgeCategory::Examples,
        },
        DocumentSpec {
            id: "routekit-compiler-pilcrow-tags",
            title: "Pilcrow tag transpilation: <pilcrow:image>, <pilcrow:head> stripping, island tags (compiler.rs)",
            path: "crates/routekit/src/templating/compiler.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "routekit-react-islands",
            title: "React island transpilation, Vite build, and asset manifest generation",
            path: "crates/routekit/src/templating/react.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "runtime-react-islands-loader",
            title: "React island browser loader",
            path: "crates/runtime/assets/react-islands.js",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "routekit-pipeline-templating",
            title: "Build pipeline: slot expansion, <pilcrow:head> hoisting, island detection, layout chain (pipeline.rs)",
            path: "crates/routekit/src/templating/pipeline.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "routekit-tests",
            title: "Routekit codegen tests",
            path: "crates/routekit/src/templating/codegen/tests.rs",
            category: KnowledgeCategory::Tests,
        },
        DocumentSpec {
            id: "routekit-pipeline-tests",
            title: "Routekit pipeline tests, including configured fragments",
            path: "crates/routekit/src/templating/pipeline.rs",
            category: KnowledgeCategory::Tests,
        },
        DocumentSpec {
            id: "routekit-markdown",
            title: "Markdown transpiler: GFM .md/.mdx → Askama HTML (markdown.rs)",
            path: "crates/routekit/src/templating/markdown.rs",
            category: KnowledgeCategory::Routekit,
        },
        DocumentSpec {
            id: "runtime-context-tests",
            title: "Runtime context tests",
            path: "crates/runtime/tests/context.rs",
            category: KnowledgeCategory::Tests,
        },
        DocumentSpec {
            id: "runtime-response-tests",
            title: "Runtime response tests",
            path: "crates/runtime/tests/response.rs",
            category: KnowledgeCategory::Tests,
        },
        DocumentSpec {
            id: "runtime-sse",
            title: "SSE: SilcrowEvent, SseEmitter, sse_stream, with_mutation_id",
            path: "crates/runtime/src/sse/server_sent_events.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-ws",
            title: "WebSocket: WsEvent, WsStream, ws(), with_mutation_id",
            path: "crates/runtime/src/ws/ws.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-response-headers",
            title: "Silcrow typed response headers (SilcrowTarget, SilcrowMutationId, etc.)",
            path: "crates/runtime/src/response/headers.rs",
            category: KnowledgeCategory::RuntimeApi,
        },
        DocumentSpec {
            id: "runtime-sse-tests",
            title: "SSE event tests",
            path: "crates/runtime/tests/sse_events.rs",
            category: KnowledgeCategory::Tests,
        },
        DocumentSpec {
            id: "runtime-ws-tests",
            title: "WebSocket event tests",
            path: "crates/runtime/tests/ws_events.rs",
            category: KnowledgeCategory::Tests,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;

    fn root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn loads_authoritative_documents() {
        let kb = KnowledgeBase::load(&root()).unwrap();
        assert!(kb.documents.iter().any(|doc| doc.path == "CLAUDE.md"));
        assert!(kb
            .documents
            .iter()
            .any(|doc| doc.category == KnowledgeCategory::Examples));
    }

    #[test]
    fn explains_registry_feature_with_evidence() {
        let root = root();
        let kb = KnowledgeBase::load(&root).unwrap();
        let registry = Registry::load_from_project(&root).unwrap();
        let explanation = kb.explain_feature(&registry, "actions").unwrap();
        assert_eq!(explanation.feature.id, "actions");
        assert!(!explanation.evidence.is_empty());
    }

    #[test]
    fn delegates_exact_silcrow_runtime_questions() {
        let root = root();
        let kb = KnowledgeBase::load(&root).unwrap();
        let registry = Registry::load_from_project(&root).unwrap();
        let answer = kb.answer_question(&registry, "How does s-boost patch the DOM?");
        assert!(answer.silcrow_delegation.is_some());
    }

    #[test]
    fn document_specs_are_unique_and_paths_exist() {
        let root = root();
        let specs = document_specs();
        let mut ids = std::collections::HashSet::new();
        for spec in &specs {
            assert!(ids.insert(spec.id), "duplicate document id: {}", spec.id);
            assert!(
                root.join(spec.path).exists(),
                "document path does not exist: {}",
                spec.path
            );
        }
    }
}
