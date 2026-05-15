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
    fn isr_constants_are_stripped_from_emitted_module() {
        let frontmatter = r#"
pub const REVALIDATE: u64 = 60;
pub const MAX_STALE: u64 = 3600;
pub const CACHE_TAGS: &[&str] = &["products", "inventory"];
pub const CACHE_VARY: &[&str] = &["tenant_id"];
pub const PRERENDER: bool = true;

pub struct Props {
    pub products: Vec<String>,
}

pub async fn load(_req: pilcrow_web::Req) -> pilcrow_web::AppResult<Props> {
    Ok(Props { products: vec![] })
}
"#;
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_products".to_string(),
            render_symbol: "render_page_products".to_string(),
            source_path: "/tmp/src/pages/products.html".to_string(),
            rust_frontmatter: frontmatter.to_string(),
            template_source: "{% for p in products %}<p>{{ p }}</p>{% endfor %}".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
        }])
        .expect("should generate with ISR constants");

        let src = &generated.source;

        // ISR constants must not appear in the emitted module source.
        assert!(
            !src.contains("REVALIDATE"),
            "REVALIDATE leaked into emitted source"
        );
        assert!(
            !src.contains("MAX_STALE"),
            "MAX_STALE leaked into emitted source"
        );
        assert!(
            !src.contains("CACHE_TAGS"),
            "CACHE_TAGS leaked into emitted source"
        );
        assert!(
            !src.contains("CACHE_VARY"),
            "CACHE_VARY leaked into emitted source"
        );
        assert!(
            !src.contains("PRERENDER"),
            "PRERENDER leaked into emitted source"
        );

        // Props and load() must still be emitted.
        assert!(src.contains("pub struct Props"), "Props was stripped");
        assert!(src.contains("pub async fn load"), "load() was stripped");

        // ISR config must be recorded in the ISR map.
        let isr = generated
            .isr_config_map
            .get("page_products")
            .expect("page_products should have ISR config");
        assert_eq!(isr.revalidate, Some(60));
        assert_eq!(isr.max_stale, Some(3600));
        assert_eq!(
            isr.cache_tags,
            vec!["products".to_string(), "inventory".to_string()]
        );
        assert_eq!(isr.cache_vary, vec!["tenant_id".to_string()]);

        // PRERENDER is now in the SSG map, not in ISrOpts.
        let ssg = generated
            .ssg_config_map
            .get("page_products")
            .expect("page_products should have SSG config");
        assert!(ssg.prerender);
    }

    #[test]
    fn ssg_prerender_constant_is_stripped_and_recorded_in_ssg_map() {
        let frontmatter = r#"
pub const PRERENDER: bool = true;

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
        }])
        .expect("should generate with PRERENDER constant");

        // PRERENDER must not appear in the emitted source.
        assert!(
            !generated.source.contains("PRERENDER"),
            "PRERENDER leaked into emitted source"
        );

        // SSG config must be recorded.
        let ssg = generated
            .ssg_config_map
            .get("page_about")
            .expect("page_about should have SSG config");
        assert!(ssg.prerender);
        assert!(!ssg.has_entries_fn);

        // Must NOT be in the ISR map.
        assert!(!generated.isr_config_map.contains_key("page_about"));
    }

    #[test]
    fn entries_fn_is_detected_in_ssg_opts() {
        let frontmatter = r#"
pub const PRERENDER: bool = true;

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
        }])
        .expect("should generate with entries fn");

        let ssg = generated
            .ssg_config_map
            .get("page_products_id")
            .expect("page_products_id should have SSG config");
        assert!(ssg.prerender);
        assert!(ssg.has_entries_fn);
    }

    #[test]
    fn non_isr_page_has_no_isr_config() {
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_index".to_string(),
            render_symbol: "render_page_index".to_string(),
            source_path: "/tmp/src/pages/index.html".to_string(),
            rust_frontmatter: "pub struct Props { pub title: String }".to_string(),
            template_source: "<h1>{{ title }}</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
        }])
        .expect("should generate");
        assert!(!generated.isr_config_map.contains_key("page_index"));
    }

    #[test]
    fn streaming_constant_is_stripped_and_recorded_in_page_options() {
        let frontmatter = r#"
pub const STREAMING: bool = true;
pub struct Props { pub title: String }
pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props { title: "hi".into() })
}
"#;
        let generated = render_generated_templates_module(&[TemplateCodegenInput {
            module_name: "page_about".to_string(),
            render_symbol: "render_page_about".to_string(),
            source_path: "/tmp/src/pages/about.html".to_string(),
            rust_frontmatter: frontmatter.to_string(),
            template_source: "<h1>:text</h1>".to_string(),
            layout_chain: vec![],
            fragment_url_prefix: None,
            route_params: vec![],
        }])
        .expect("should generate with STREAMING constant");

        // STREAMING must not appear in the emitted source.
        assert!(
            !generated.source.contains("STREAMING"),
            "STREAMING leaked into emitted source"
        );

        // streaming flag must be recorded in page_options.
        let opts = generated
            .page_options
            .get("page_about")
            .expect("page_about in options");
        assert!(opts.streaming);

        // Props::default() must be derivable (Default injected automatically).
        assert!(
            generated.source.contains("derive"),
            "Default should be injected for streaming"
        );
    }

    #[test]
    fn streaming_handler_emits_spawn_and_patch_script() {
        let page_route = GeneratedPageRoute {
            pattern: "/products".to_string(),
            template_path: "/tmp/src/pages/products.html".to_string(),
            symbol: "page_products".to_string(),
            render_symbol: "render_page_products".to_string(),
            route_params: vec![],
            param_matchers: HashMap::new(),
        };
        let load_sig = LoadSignature {
            is_async: true,
            returns_result: true,
            wants_client: false,
            wants_req: true,
            wants_page: false,
            wants_live: false,
        };
        let mut load_map: HashMap<String, Option<LoadSignature>> = HashMap::new();
        load_map.insert("page_products".to_string(), Some(load_sig));

        let mut page_opts: HashMap<String, PageOptions> = HashMap::new();
        page_opts.insert(
            "page_products".to_string(),
            PageOptions {
                streaming: true,
                ..Default::default()
            },
        );

        let source = render_generated_app_module(
            &[page_route],
            &[],
            &AppCodegenMaps {
                load_map: &load_map,
                layout_fields_map: &HashMap::new(),
                error_module_for_page: &HashMap::new(),
                not_found_module: None,
                loading_module_for_page: &HashMap::new(),
                action_map: &HashMap::new(),
                page_options_map: &page_opts,
                deferred_fields_map: &HashMap::new(),
                deferred_html_fields_map: &HashMap::new(),
                isr_config_map: &HashMap::new(),
                ssg_config_map: &HashMap::new(),
                live_fields_map: &HashMap::new(),
                has_live_fn_map: &HashMap::new(),
                fsr_live_source_map: &HashMap::new(),
                fsr_live_fields_map: &HashMap::new(),
            },
            HookFlags::default(),
            false,
            false,
        )
        .expect("streaming app module should render");

        // Must spawn page load in background.
        assert!(
            source.contains("tokio::spawn"),
            "STREAMING handler must spawn page load"
        );
        // Must use __streaming_props_response.
        assert!(
            source.contains("__streaming_props_response"),
            "must use streaming response"
        );
        // Must inject the window.__ps shim.
        assert!(source.contains("window.__ps"), "must inject streaming shim");
        // Must serialize page props.
        assert!(
            source.contains("__serialize_page_props"),
            "must serialize props"
        );
        // Must apply resp_handle.
        assert!(
            source.contains("__resp_handle.apply_to"),
            "must apply response handle"
        );
    }

    #[test]
    fn render_generated_app_module_reports_invalid_streaming_config() {
        let page_route = GeneratedPageRoute {
            pattern: "/products".to_string(),
            template_path: "/tmp/src/pages/products.html".to_string(),
            symbol: "page_products".to_string(),
            render_symbol: "render_page_products".to_string(),
            route_params: vec![],
            param_matchers: HashMap::new(),
        };
        let load_sig = LoadSignature {
            is_async: true,
            returns_result: true,
            wants_client: false,
            wants_req: true,
            wants_page: false,
            wants_live: false,
        };
        let mut load_map: HashMap<String, Option<LoadSignature>> = HashMap::new();
        load_map.insert("page_products".to_string(), Some(load_sig));

        let mut page_opts: HashMap<String, PageOptions> = HashMap::new();
        page_opts.insert(
            "page_products".to_string(),
            PageOptions {
                streaming: true,
                ..Default::default()
            },
        );
        let mut isr_opts: HashMap<String, IsrOpts> = HashMap::new();
        isr_opts.insert(
            "page_products".to_string(),
            IsrOpts {
                revalidate: Some(60),
                ..Default::default()
            },
        );

        let err = render_generated_app_module(
            &[page_route],
            &[],
            &AppCodegenMaps {
                load_map: &load_map,
                layout_fields_map: &HashMap::new(),
                error_module_for_page: &HashMap::new(),
                not_found_module: None,
                loading_module_for_page: &HashMap::new(),
                action_map: &HashMap::new(),
                page_options_map: &page_opts,
                deferred_fields_map: &HashMap::new(),
                deferred_html_fields_map: &HashMap::new(),
                isr_config_map: &isr_opts,
                ssg_config_map: &HashMap::new(),
                live_fields_map: &HashMap::new(),
                has_live_fn_map: &HashMap::new(),
                fsr_live_source_map: &HashMap::new(),
                fsr_live_fields_map: &HashMap::new(),
            },
            HookFlags::default(),
            false,
            false,
        )
        .expect_err("invalid streaming + ISR config should be reported");

        let message = err.to_string();
        assert!(message.contains("invalid Pilcrow route configuration"));
        assert!(message.contains("route: /products"));
        assert!(message.contains("STREAMING = true is incompatible with REVALIDATE"));
        assert!(message.contains("suggested fix:"));
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
