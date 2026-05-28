use super::*;

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn build_generated_page_manifest_from_html_tree() {
        let root = mk_temp_root("manifest");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/about.html"), "<h1>About</h1>");
        write_file(&src.join("pages/posts/[id].html"), "<h1>Post</h1>");

        let entries = build_generated_page_manifest(&src, &[], &[]).expect("manifest should build");
        let patterns = entries
            .iter()
            .map(|e| e.pattern.as_str())
            .collect::<Vec<_>>();

        assert!(patterns.contains(&"/"));
        assert!(patterns.contains(&"/about"));
        assert!(patterns.contains(&"/posts/:id"));
        assert!(entries.iter().any(|e| e.symbol == "page_posts_id"));
        assert!(
            entries
                .iter()
                .any(|e| e.render_symbol == "render_page_posts_id")
        );

        cleanup(&root);
    }

    #[test]
    fn render_generated_routes_module_contains_helpers() {
        let entries = vec![GeneratedPageRoute {
            pattern: "/about".to_string(),
            template_path: "/tmp/src/pages/about.html".to_string(),
            symbol: "page_about".to_string(),
            render_symbol: "render_page_about".to_string(),
            route_params: vec![],
            param_matchers: HashMap::new(),
        }];

        let source = render_generated_routes_module(&entries);
        assert!(source.contains("pub struct GeneratedPageRoute"));
        assert!(source.contains("GENERATED_PAGE_ROUTES"));
        assert!(source.contains("pub fn generated_routes()"));
        assert!(source.contains("pub fn register_generated_routes"));
        assert!(source.contains("pub fn pilcrow_router"));
        assert!(source.contains("page_about"));
        assert!(source.contains("render_page_about"));
    }

    #[test]
    fn write_generated_routes_module_writes_file() {
        let root = mk_temp_root("write_module");
        let src = root.join("src");
        let out_file = root.join("out/generated_routes.rs");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/blog/[slug].html"), "<h1>Blog</h1>");

        let entries = write_generated_routes_module(&src, &out_file, &[], &[])
            .expect("should write generated file");
        assert_eq!(entries.len(), 2);
        assert!(out_file.exists());

        let generated = fs::read_to_string(&out_file).expect("read generated");
        assert!(generated.contains("/blog/:slug"));
        assert!(generated.contains("page_blog_slug"));
        assert!(generated.contains("render_page_blog_slug"));

        cleanup(&root);
    }

    #[test]
    fn render_generated_templates_module_instruments_props_and_render_fn() {
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_index".to_string(),
            render_symbol: "render_page_index".to_string(),
            source_path: "/tmp/src/pages/index.html".to_string(),
            rust_frontmatter: "pub struct Props { pub title: String }".to_string(),
            template_source: "<h1>{{ title }}</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect("template module should generate");

        assert_eq!(generated.entries.len(), 1);
        assert!(generated.source.contains("pub mod page_index"));
        assert!(generated.source.contains("askama :: Template"));
        assert!(generated.source.contains("serde :: Serialize"));
        assert!(generated.source.contains("<h1>{{ title }}</h1>"));
        assert!(generated.source.contains("pub fn render_page_index"));
        assert!(generated.source.contains("GENERATED_TEMPLATES"));
    }

    #[test]
    fn render_generated_templates_module_synthesizes_missing_props() {
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_index".to_string(),
            render_symbol: "render_page_index".to_string(),
            source_path: "/tmp/src/pages/index.html".to_string(),
            rust_frontmatter: "pub fn helper() {}".to_string(),
            template_source: "<h1>Home</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect("synthesized Props should compile");

        // A unit struct is synthesized when the frontmatter omits Props.
        assert!(generated.source.contains("pub struct Props"));
        assert!(generated.source.contains("template (source"));
        // Static page (no load fn) → entry is None.
        assert_eq!(generated.load_map.get("page_index"), Some(&None));
    }

    #[test]
    fn render_generated_templates_module_emits_page_params_alias() {
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_products_id".to_string(),
            render_symbol: "render_page_products_id".to_string(),
            source_path: "/tmp/src/pages/products/[id:int].html".to_string(),
            rust_frontmatter: "pub struct Props { pub id: i64 }\npub async fn load(ctx: Page) -> AppResult<Props> { Ok(Props { id: ctx.params.id }) }".to_string(),
            template_source: "<h1>{{ id }}</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![GeneratedRouteParam {
                name: "id".to_string(),
                rust_type: "i64".to_string(),
                optional: false,
                catch_all: false,
            }],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect("typed page params should generate");

        assert!(generated.source.contains("pub struct Params"));
        assert!(generated.source.contains("pub id: i64"));
        assert!(
            generated
                .source
                .contains("pub type Page = ::pilcrow_web::Page<Params>")
        );
        assert!(generated.source.contains("parse::<i64>"));
    }

    #[test]
    fn render_generated_templates_module_fails_with_duplicate_props() {
        let err = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_index".to_string(),
            render_symbol: "render_page_index".to_string(),
            source_path: "/tmp/src/pages/index.html".to_string(),
            rust_frontmatter: "pub struct Props {} pub struct Props { pub id: i64 }".to_string(),
            template_source: "<h1>Home</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect_err("duplicate props should fail");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(
            err.to_string()
                .contains("declares multiple `Props` structs")
        );
    }

    #[test]
    fn render_generated_templates_module_validates_fsr_live_slots_against_live_rs() {
        let root = mk_temp_root("fsr_live_validation");
        let page_path = root.join("src/pages/tickets/index.html");
        let live_path = root.join("src/pages/tickets/live.rs");
        write_file(
            &live_path,
            r#"
use pilcrow_web::live::*;

pub struct Live {
    pub status: LiveProp<String>,
}
"#,
        );

        let err = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_tickets".to_string(),
            render_symbol: "render_page_tickets".to_string(),
            source_path: page_path.display().to_string(),
            rust_frontmatter: "pub struct Props { pub status: String }".to_string(),
            template_source: r#"<span s-live="ticket_status">{{ status }}</span>"#.to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect_err("mismatched FSR live slots should fail");

        cleanup(&root);

        let message = err.to_string();
        assert!(message.contains("failed to validate"));
        assert!(message.contains("s-live=\"ticket_status\" exists"));
        assert!(message.contains("no matching Live field exists"));
        assert!(message.contains("Live field `status` exists"));
        assert!(message.contains("#[pilcrow::allow_unused]"));
    }

    #[test]
    fn fsr_script_routes_objects_to_silcrow_publish() {
        let script = super::app_module::fsr_patch_script();
        assert!(
            script.contains("window.Silcrow&&window.Silcrow.publish"),
            "expected Silcrow.publish call in FSR script"
        );
        assert!(
            script.contains("typeof v==='object'"),
            "expected typeof object check in FSR script"
        );
    }

    #[test]
    fn fsr_script_still_patches_scalars_via_s_live() {
        let script = super::app_module::fsr_patch_script();
        assert!(script.contains("s-live"), "expected s-live selector");
        assert!(
            script.contains("n.textContent=v==null"),
            "expected scalar textContent assignment in FSR script"
        );
    }

    #[test]
    fn live_anchor_emits_data_pilcrow_live_element() {
        use super::app_module::{AppCodegenMaps, render_generated_app_module};
        use crate::templating::codegen::{GeneratedPageRoute, HookFlags};
        use std::collections::{HashMap, HashSet};

        let page_entry = GeneratedPageRoute {
            pattern: "/dashboard".to_string(),
            template_path: "/tmp/src/pages/dashboard.html".to_string(),
            symbol: "page_dashboard".to_string(),
            render_symbol: "render_page_dashboard".to_string(),
            route_params: vec![],
            param_matchers: HashMap::new(),
        };

        let mut live_fields_map: HashMap<String, Vec<String>> = HashMap::new();
        live_fields_map.insert("page_dashboard".to_string(), vec!["counter".to_string()]);

        let mut load_map: HashMap<String, Option<crate::templating::codegen::LoadSignature>> =
            HashMap::new();
        load_map.insert(
            "page_dashboard".to_string(),
            Some(crate::templating::codegen::LoadSignature {
                is_async: true,
                wants_client: false,
                wants_req: true,
                wants_page: false,
                wants_live: false,
                returns_result: true,
            }),
        );

        let maps = AppCodegenMaps {
            load_map: &load_map,
            layout_fields_map: &HashMap::new(),
            error_module_for_page: &HashMap::new(),
            not_found_module: None,
            loading_module_for_page: &HashMap::new(),
            action_map: &HashMap::new(),
            page_options_map: &HashMap::new(),
            live_fields_map: &live_fields_map,
            has_live_fn_map: &HashMap::new(),
            fsr_live_source_map: &HashMap::new(),
            fsr_live_fields_map: &HashMap::new(),
            fsr_default_revalidate_symbols: &HashSet::new(),
            layout_chain_ids_map: &HashMap::new(),
            page_slot_map: &HashMap::new(),
        };

        let source = render_generated_app_module(
            &[page_entry],
            &[],
            &maps,
            HookFlags {
                has_handle: false,
                has_handle_error: false,
                has_init: false,
            },
            false,
            false,
        )
        .expect("render_generated_app_module should succeed");

        assert!(
            source.contains("data-pilcrow-live"),
            "expected data-pilcrow-live anchor element, got:\n{source}"
        );
        assert!(
            !source.contains("__LIVE_SHIM"),
            "__LIVE_SHIM should not appear in generated source"
        );
        assert!(
            !source.contains("window.__pilcrow_live_patch"),
            "inline patch function should not appear in generated source"
        );
    }

    fn mk_temp_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pilcrow_routekit_codegen_{}_{}_{}",
            prefix,
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write file");
    }

    fn cleanup(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("cleanup temp dir");
        }
    }

    #[test]
    fn build_generated_api_manifest_from_rs_tree() {
        let root = mk_temp_root("api_manifest");
        let src = root.join("src");

        write_file(&src.join("api/todos.rs"), "pub fn router() {}");
        write_file(&src.join("api/users/[id].rs"), "pub fn router() {}");

        let entries = build_generated_api_manifest(&src, &[]).expect("manifest should build");
        let patterns = entries
            .iter()
            .map(|e| e.pattern.as_str())
            .collect::<Vec<_>>();

        assert!(patterns.contains(&"/api/todos"));
        assert!(patterns.contains(&"/api/users/:id"));
        assert!(entries.iter().any(|e| e.symbol == "api_todos"));
        assert!(entries.iter().any(|e| e.symbol == "api_users_id"));
        assert!(entries.iter().any(|e| e.module_path == "api::users::id"));

        cleanup(&root);
    }

    #[test]
    fn render_generated_api_routes_module_contains_helpers() {
        let entries = vec![GeneratedApiRoute {
            pattern: "/api/todos".to_string(),
            module_path: "api::todos".to_string(),
            symbol: "api_todos".to_string(),
        }];

        let source = render_generated_api_routes_module(&entries);
        assert!(source.contains("pub struct GeneratedApiRoute"));
        assert!(source.contains("GENERATED_API_ROUTES"));
        assert!(source.contains("pub fn generated_api_routes()"));
        assert!(source.contains("pub fn register_generated_api_routes"));
        assert!(source.contains("GENERATED_API_ROUTES.iter().fold(router, register)"));
        assert!(source.contains("api_todos"));
        assert!(source.contains("/api/todos"));
    }

    #[test]
    fn write_generated_api_routes_module_writes_file() {
        let root = mk_temp_root("write_api_module");
        let src = root.join("src");
        let out_file = root.join("out/generated_api_routes.rs");

        write_file(&src.join("api/todos.rs"), "pub fn router() {}");
        write_file(&src.join("api/users/[id].rs"), "pub fn router() {}");

        let entries = write_generated_api_routes_module(&src, &out_file, &[])
            .expect("should write generated file");
        assert_eq!(entries.len(), 2);
        assert!(out_file.exists());

        let generated = fs::read_to_string(&out_file).expect("read generated");
        assert!(generated.contains("/api/todos"));
        assert!(generated.contains("/api/users/:id"));
        assert!(generated.contains("api_users_id"));

        cleanup(&root);
    }

    #[test]
    fn emit_action_route_emits_named_dispatch_table() {
        let actions = vec![
            ActionFn {
                name: "create".to_string(),
                is_async: true,
                returns_result: true,
                wants_req: true,
            },
            ActionFn {
                name: "delete".to_string(),
                is_async: true,
                returns_result: true,
                wants_req: true,
            },
        ];

        let source = emit_action_route(&actions, "/items", "page_items", None);

        // POST handler shape
        assert!(source.contains(".route(\"/items\""));
        assert!(source.contains("::pilcrow_web::axum::routing::post"));
        assert!(source.contains("let __resp_handle = req.res.clone();"));
        assert!(source.contains("let __action = req.action().to_owned();"));
        assert!(source.contains("match __action.as_str()"));

        // Per-action dispatch arms calling the code-behind fns
        assert!(
            source.contains("\"create\" => match __pilcrow_gen::page_items::create(req).await")
        );
        assert!(
            source.contains("\"delete\" => match __pilcrow_gen::page_items::delete(req).await")
        );

        // Unknown action → 404 via AppError::NotFound
        assert!(source.contains("_ => {"));
        assert!(source.contains("::pilcrow_web::AppError::NotFound"));
        assert!(source.contains("unknown action:"));

        // Redirect short-circuit still present and __resp_handle applied
        assert!(source.contains("::pilcrow_web::AppError::Redirect"));
        assert!(source.contains("__resp_handle.apply_to(&mut __response);"));
    }

    #[test]
    fn promote_after_constant_is_stripped_and_recorded_in_page_options() {
        let frontmatter = r#"
pub const PROMOTE_AFTER: u32 = 0;

pub struct Props {
    pub title: String,
}

pub async fn load(_req: pilcrow_web::Req) -> pilcrow_web::AppResult<Props> {
    Ok(Props { title: "Hello".to_string() })
}
"#;
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_about".to_string(),
            render_symbol: "render_page_about".to_string(),
            source_path: "/tmp/src/pages/about.html".to_string(),
            rust_frontmatter: frontmatter.to_string(),
            template_source: "<h1>{{ title }}</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect("should generate with PROMOTE_AFTER constant");

        // PROMOTE_AFTER must not appear in the emitted source.
        assert!(
            !generated.source.contains("PROMOTE_AFTER"),
            "PROMOTE_AFTER leaked into emitted source"
        );

        // promote_after must be recorded in page_options.
        let opts = generated
            .page_options
            .get("page_about")
            .expect("page_about should have page options");
        assert_eq!(opts.fsr.promote_after, Some(0));
    }

    #[test]
    fn entries_fn_is_detected_in_ssg_opts() {
        let frontmatter = r#"
pub const PROMOTE_AFTER: u32 = 0;

pub struct Props {
    pub name: String,
}

pub async fn entries() -> Vec<::std::collections::HashMap<String, String>> {
    vec![]
}

pub async fn load(_req: pilcrow_web::Req) -> pilcrow_web::AppResult<Props> {
    Ok(Props { name: "test".to_string() })
}
"#;
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_products_id".to_string(),
            render_symbol: "render_page_products_id".to_string(),
            source_path: "/tmp/src/pages/products/[id].html".to_string(),
            rust_frontmatter: frontmatter.to_string(),
            template_source: "<h1>{{ name }}</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect("should generate with entries fn");

        let opts = generated
            .page_options
            .get("page_products_id")
            .expect("page_products_id should have page options");
        assert_eq!(opts.fsr.promote_after, Some(0));
        assert!(opts.ssg.has_entries_fn);
    }

    #[test]
    fn page_without_promote_after_has_no_fsr_promote_threshold() {
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_index".to_string(),
            render_symbol: "render_page_index".to_string(),
            source_path: "/tmp/src/pages/index.html".to_string(),
            rust_frontmatter: "pub struct Props { pub title: String }".to_string(),
            template_source: "<h1>{{ title }}</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect("should generate");

        let opts = generated.page_options.get("page_index");
        let promote_after = opts.map(|o| o.fsr.promote_after).unwrap_or(None);
        assert!(
            promote_after.is_none(),
            "expected no promote_after threshold for a plain page"
        );
    }

    #[test]
    fn revalidate_attr_stripped_from_props_and_recorded_in_live_field_attrs() {
        // Build a frontmatter that puts #[revalidate(N)] on a Props LiveProp field.
        let frontmatter = r#"
pub struct Props {
    #[revalidate(60)]
    pub status: LiveProp<String>,
    pub title: String,
}

pub async fn load(_req: pilcrow_web::Req) -> pilcrow_web::AppResult<Props> {
    Ok(Props { status: Default::default(), title: "Hello".to_string() })
}
"#;
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_tickets".to_string(),
            render_symbol: "render_page_tickets".to_string(),
            source_path: "/tmp/src/pages/tickets/index.html".to_string(),
            rust_frontmatter: frontmatter.to_string(),
            template_source: "<h1>{{ title }}</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect("should generate with revalidate attr");

        // Attr must be stripped from emitted source.
        assert!(
            !generated.source.contains("revalidate"),
            "revalidate attr leaked into emitted source:\n{}",
            generated.source
        );

        // Must be recorded in page_options.
        let opts = generated
            .page_options
            .get("page_tickets")
            .expect("page_tickets should have page options");
        let status_attr = opts
            .fsr
            .live_field_attrs
            .get("status")
            .expect("status should have LiveFieldAttr");
        assert_eq!(status_attr.revalidate_secs, Some(60));

        // title has no attr — should not appear in live_field_attrs.
        assert!(opts.fsr.live_field_attrs.get("title").is_none());
    }

    #[test]
    fn malformed_revalidate_attr_is_a_build_error() {
        let frontmatter = r#"
pub struct Props {
    #[revalidate("60")]
    pub status: LiveProp<String>,
}

pub async fn load(_req: pilcrow_web::Req) -> pilcrow_web::AppResult<Props> {
    Ok(Props { status: Default::default() })
}
"#;
        let err = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_tickets".to_string(),
            render_symbol: "render_page_tickets".to_string(),
            source_path: "/tmp/src/pages/tickets/index.html".to_string(),
            rust_frontmatter: frontmatter.to_string(),
            template_source: "<h1>hi</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect_err("malformed #[revalidate] should fail");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let msg = err.to_string();
        assert!(
            msg.contains("failed to parse `#[revalidate(...)]`"),
            "got: {msg}"
        );
        assert!(msg.contains("status"), "got: {msg}");
    }

    #[test]
    fn malformed_depends_on_attr_is_a_build_error() {
        let frontmatter = r#"
pub struct Props {
    #[depends_on(123)]
    pub status: LiveProp<String>,
}

pub async fn load(_req: pilcrow_web::Req) -> pilcrow_web::AppResult<Props> {
    Ok(Props { status: Default::default() })
}
"#;
        let err = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_tickets".to_string(),
            render_symbol: "render_page_tickets".to_string(),
            source_path: "/tmp/src/pages/tickets/index.html".to_string(),
            rust_frontmatter: frontmatter.to_string(),
            template_source: "<h1>hi</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect_err("malformed #[depends_on] should fail");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let msg = err.to_string();
        assert!(
            msg.contains("failed to parse `#[depends_on(...)]`"),
            "got: {msg}"
        );
        assert!(msg.contains("status"), "got: {msg}");
    }

    #[test]
    fn scheduled_invalidations_emitted_in_pilcrow_init() {
        use super::app_module::{AppCodegenMaps, render_generated_app_module};
        use crate::templating::codegen::{GeneratedPageRoute, HookFlags};
        use crate::templating::page_options::{LiveFieldAttr, PageOptions};
        use std::collections::{HashMap, HashSet};

        let page_entry = GeneratedPageRoute {
            pattern: "/tickets".to_string(),
            template_path: "/tmp/src/pages/tickets/index.html".to_string(),
            symbol: "page_tickets".to_string(),
            render_symbol: "render_page_tickets".to_string(),
            route_params: vec![],
            param_matchers: HashMap::new(),
        };

        let mut live_field_attrs = HashMap::new();
        live_field_attrs.insert(
            "status".to_string(),
            LiveFieldAttr {
                revalidate_secs: Some(60),
                depends_on: None,
            },
        );

        let mut page_opts = PageOptions::default();
        page_opts.fsr.live_field_attrs = live_field_attrs;

        let mut page_options_map = HashMap::new();
        page_options_map.insert("page_tickets".to_string(), page_opts);

        let mut load_map = HashMap::new();
        load_map.insert(
            "page_tickets".to_string(),
            None::<crate::templating::codegen::LoadSignature>,
        );

        let maps = AppCodegenMaps {
            load_map: &load_map,
            layout_fields_map: &HashMap::new(),
            error_module_for_page: &HashMap::new(),
            not_found_module: None,
            loading_module_for_page: &HashMap::new(),
            action_map: &HashMap::new(),
            page_options_map: &page_options_map,
            live_fields_map: &HashMap::new(),
            has_live_fn_map: &HashMap::new(),
            fsr_live_source_map: &HashMap::new(),
            fsr_live_fields_map: &HashMap::new(),
            fsr_default_revalidate_symbols: &HashSet::new(),
            layout_chain_ids_map: &HashMap::new(),
            page_slot_map: &HashMap::new(),
        };

        let source = render_generated_app_module(
            &[page_entry],
            &[],
            &maps,
            HookFlags {
                has_handle: false,
                has_handle_error: false,
                has_init: false,
            },
            false,
            false,
        )
        .expect("render_generated_app_module should succeed");

        assert!(
            source.contains("__register_codegen_scheduled_invalidations"),
            "expected __register_codegen_scheduled_invalidations in __pilcrow_init:\n{source}"
        );
        assert!(
            source.contains("ScheduledInvalidation::new"),
            "expected ScheduledInvalidation::new in __pilcrow_init:\n{source}"
        );
        assert!(
            source.contains("\"page_tickets::__revalidate_60s\""),
            "expected shared synthetic dep key page_tickets::__revalidate_60s in scheduled invalidation:\n{source}"
        );
        assert!(
            source.contains("from_secs(60u64)"),
            "expected 60s interval in scheduled invalidation:\n{source}"
        );
    }

    #[test]
    fn default_revalidate_routes_emitted_when_fields_lack_explicit_revalidate() {
        use super::app_module::{AppCodegenMaps, render_generated_app_module};
        use crate::templating::codegen::{GeneratedPageRoute, HookFlags};
        use std::collections::{HashMap, HashSet};

        let page_entry = GeneratedPageRoute {
            pattern: "/dash".to_string(),
            template_path: "/tmp/src/pages/dash/index.html".to_string(),
            symbol: "page_dash".to_string(),
            render_symbol: "render_page_dash".to_string(),
            route_params: vec![],
            param_matchers: HashMap::new(),
        };

        let mut load_map = HashMap::new();
        load_map.insert(
            "page_dash".to_string(),
            None::<crate::templating::codegen::LoadSignature>,
        );

        let mut default_routes = HashSet::new();
        default_routes.insert("page_dash".to_string());

        let maps = AppCodegenMaps {
            load_map: &load_map,
            layout_fields_map: &HashMap::new(),
            error_module_for_page: &HashMap::new(),
            not_found_module: None,
            loading_module_for_page: &HashMap::new(),
            action_map: &HashMap::new(),
            page_options_map: &HashMap::new(),
            live_fields_map: &HashMap::new(),
            has_live_fn_map: &HashMap::new(),
            fsr_live_source_map: &HashMap::new(),
            fsr_live_fields_map: &HashMap::new(),
            fsr_default_revalidate_symbols: &default_routes,
            layout_chain_ids_map: &HashMap::new(),
            page_slot_map: &HashMap::new(),
        };

        let source = render_generated_app_module(
            &[page_entry],
            &[],
            &maps,
            HookFlags {
                has_handle: false,
                has_handle_error: false,
                has_init: false,
            },
            false,
            false,
        )
        .expect("render_generated_app_module should succeed");

        assert!(
            source.contains("__register_codegen_default_revalidate_routes"),
            "expected __register_codegen_default_revalidate_routes in __pilcrow_init:\n{source}"
        );
        assert!(
            source.contains("pub fn build_router()")
                && source.contains("__pilcrow_register_codegen_revalidation();"),
            "expected build_router to register default revalidation before direct start():\n{source}"
        );
        assert!(
            source.contains("\"page_dash\""),
            "expected page_dash route name in default revalidate registration:\n{source}"
        );
        assert!(
            !source.contains("__register_codegen_scheduled_invalidations"),
            "no field-level timers expected when only default routes are registered:\n{source}"
        );
    }

    #[test]
    fn revalidate_and_depends_on_together_is_a_build_error() {
        let frontmatter = r#"
pub struct Props {
    #[revalidate(60)]
    #[depends_on("prices:updated")]
    pub status: LiveProp<String>,
}

pub async fn load(_req: pilcrow_web::Req) -> pilcrow_web::AppResult<Props> {
    Ok(Props { status: Default::default() })
}
"#;
        let err = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_tickets".to_string(),
            render_symbol: "render_page_tickets".to_string(),
            source_path: "/tmp/src/pages/tickets/index.html".to_string(),
            rust_frontmatter: frontmatter.to_string(),
            template_source: "<h1>hi</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect_err("#[revalidate] and #[depends_on] on the same field should fail");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let msg = err.to_string();
        assert!(msg.contains("mutually exclusive"), "got: {msg}");
        assert!(msg.contains("status"), "got: {msg}");
    }

    #[test]
    fn streaming_constant_is_silently_stripped() {
        let frontmatter = "pub const STREAMING: bool = true;\npub struct Props {}\n";
        let _generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_about".to_string(),
            render_symbol: "render_page_about".to_string(),
            source_path: "/tmp/src/pages/about.html".to_string(),
            rust_frontmatter: frontmatter.to_string(),
            template_source: "<h1>hi</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
            layout_chain_ids: vec![],
            page_slot: None,
        }])
        .expect("STREAMING should be ignored, not a build error");
    }

    #[test]
    fn emit_action_route_uses_custom_error_module_when_provided() {
        let actions = vec![ActionFn {
            name: "update".to_string(),
            is_async: true,
            returns_result: true,
            wants_req: true,
        }];

        let source = emit_action_route(&actions, "/settings", "page_settings", Some("error_root"));

        // Error-page render goes through the provided error module
        assert!(source.contains("__pilcrow_gen::error_root::Props"));
        assert!(source.contains("__pilcrow_gen::error_root::render_error_root"));
        // And __resp_handle is applied to the error response too
        assert!(source.contains("__resp_handle.apply_to(&mut __err_resp);"));
    }
}
