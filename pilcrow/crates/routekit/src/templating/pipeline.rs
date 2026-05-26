use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use globset::{Glob, GlobSetBuilder};

use crate::Route;
use crate::routing::discovery::{
    DiscoveredHtmlFiles, discover_fragment_files, discover_html_files_with_fragment_dirs,
};
use crate::templating::build_config::PilcrowBuildConfig;
use crate::templating::codegen::{
    AppCodegenMaps, GeneratedApiRoute, GeneratedPageRoute, GeneratedRouteParam,
    GeneratedTemplateEntry, HookFlags, TemplateCodegenInput, build_generated_fragment_manifest,
    write_generated_api_routes_module, write_generated_app_module, write_generated_routes_module,
    write_generated_templates_module,
};
use crate::templating::compiler::{
    inject_form_method_attrs, split_html_module, transpile_component_tags, transpile_island_tags,
    transpile_pilcrow_tags,
};
use crate::templating::markdown::transpile_markdown;
use crate::templating::react::{
    ReactIslandRef, build_react_assets, replace_react_placeholders,
    replace_react_shell_placeholders, transpile_react_tags,
};
use crate::templating::solid::{
    SolidIslandRef, build_solid_assets, replace_solid_placeholders, transpile_solid_tags,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlSourceKind {
    Page,
    Ui,
    /// URL-accessible HTML fragment from a configured fragment directory.
    /// Like a page but never receives layout auto-wrapping.
    Fragment,
    /// `_layout.html` files discovered inside `src/pages/` subdirectories.
    /// They auto-wrap sibling and descendant pages; they are not routable themselves.
    AutoLayout,
    /// `_error.html` files discovered inside `src/pages/` subdirectories.
    /// Rendered when a page's `load()` returns `Err`. Not routable.
    /// Props are framework-injected: `{ status: u16, message: String }`.
    ErrorPage,
    /// `_not_found.html` files discovered inside `src/pages/`.
    /// Registered as the axum fallback handler. Not routable.
    /// Props are framework-injected: `{}`.
    NotFoundPage,
    /// `_loading.html` files discovered inside `src/pages/` subdirectories.
    /// Embedded as a `<template id="__pilcrow_loading">` in sibling/descendant pages.
    /// silcrow.js shows this skeleton immediately when navigation begins.
    /// Props are framework-injected: `{}`.
    LoadingPage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessedHtmlFile {
    pub kind: HtmlSourceKind,
    pub source_path: PathBuf,
    pub template_output_path: PathBuf,
    pub rust_frontmatter: String,
    pub transpiled_template: String,
    pub module_name: String,
    pub render_symbol: String,
    /// Ordered layout chain for this page: [outermost_auto_layout, ..., explicit_layout].
    /// Only meaningful for `Page` kind; empty for `Ui`, `Layout`, and `AutoLayout`.
    pub layout_chain: Vec<String>,
    /// URL prefix for fragment directory entries (e.g. `"widgets"`). `None` for non-fragments.
    pub fragment_url_prefix: Option<String>,
    /// Layout IDs for each auto-layout in the chain (outermost first).
    /// Only populated for page modules that have at least one auto-layout.
    pub layout_chain_ids: Vec<String>,
    /// Route pattern used as the data-ps-slot value for this page.
    /// Only set for page modules.
    pub page_slot: Option<String>,
    /// Inner content of `<pilcrow:head>` extracted before the pipeline strips it.
    /// Used to populate `<template data-ps-head>` in the post-processed template.
    pub ps_head_inner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerOutput {
    pub preprocessed_files: Vec<PreprocessedHtmlFile>,
    pub generated_routes_file: PathBuf,
    pub generated_routes: Vec<GeneratedPageRoute>,
    pub generated_templates_file: PathBuf,
    pub generated_templates: Vec<GeneratedTemplateEntry>,
    pub generated_api_routes_file: PathBuf,
    pub generated_api_routes: Vec<GeneratedApiRoute>,
    pub generated_app_file: PathBuf,
}

#[derive(Debug, Clone)]
struct FragmentSourceGroup {
    dir: PathBuf,
    url_prefix: String,
}

/// Full compile pipeline — delegates to `compile_to_out_dir_with_config` with empty config.
pub fn compile_to_out_dir(
    src_root: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
) -> io::Result<CompilerOutput> {
    compile_to_out_dir_with_config(src_root, out_dir, &PilcrowBuildConfig::default())
}

/// Full compile pipeline for Pilcrow `.html` sources with fragment directory support.
///
/// Output layout in `out_dir`:
/// - `generated_routes.rs` (route manifest + registration helpers)
/// - `generated_templates.rs` (compile-time Askama render functions)
/// - `pilcrow_templates/{pages,ui,layouts,fragments}/...` (transpiled Askama templates)
pub fn compile_to_out_dir_with_config(
    src_root: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
    build_config: &PilcrowBuildConfig,
) -> io::Result<CompilerOutput> {
    let src_root = src_root.as_ref();
    let out_dir = out_dir.as_ref();

    let configured_fragment_dirs = expand_fragment_entries(src_root, build_config)?;
    let fragment_dirs = configured_fragment_dirs
        .iter()
        .map(|group| group.dir.clone())
        .collect::<Vec<_>>();
    let discovered = discover_html_files_with_fragment_dirs(
        src_root,
        &build_config.routing.ignore_directories,
        &fragment_dirs,
    )?;
    let templates_root = out_dir.join("pilcrow_templates");
    let mut react_islands = Vec::<ReactIslandRef>::new();
    let mut solid_islands = Vec::<SolidIslandRef>::new();
    let mut files = preprocess_discovered_sources(
        src_root,
        &templates_root,
        &discovered,
        build_config,
        &mut react_islands,
        &mut solid_islands,
    )?;

    // ── Fragment groups ──────────────────────────────────────────────────────
    let mut fragment_routes: Vec<crate::templating::codegen::GeneratedPageRoute> = Vec::new();
    let mut fragment_error_module_for_route: HashMap<String, String> = HashMap::new();
    let mut fragment_loading_module_for_route: HashMap<String, String> = HashMap::new();

    for group in &configured_fragment_dirs {
        let fragment_dir = &group.dir;
        let url_prefix = group.url_prefix.clone();
        let discovered_frags = discover_fragment_files(
            src_root,
            fragment_dir,
            &build_config.routing.ignore_directories,
        )?;
        let frag_templates_root = templates_root.join("fragments").join(&url_prefix);

        // Load routable fragment HTML files into the module graph.
        let mut frag_modules: HashMap<PathBuf, HtmlModuleSource> = HashMap::new();
        load_fragment_source_group(
            src_root,
            &url_prefix,
            &discovered_frags.fragments,
            fragment_dir,
            &frag_templates_root,
            &mut frag_modules,
            build_config,
        )?;
        // Error pages within the fragment group (not routable, use fragment naming too).
        load_source_group(
            src_root,
            HtmlSourceKind::ErrorPage,
            &discovered_frags.error_pages,
            src_root,
            &frag_templates_root.join("error_pages"),
            &mut frag_modules,
            build_config,
        )?;
        load_source_group(
            src_root,
            HtmlSourceKind::LoadingPage,
            &discovered_frags.loading_pages,
            src_root,
            &frag_templates_root.join("loading_pages"),
            &mut frag_modules,
            build_config,
        )?;

        // Include ui/ modules in the graph so fragments can import <Component />.
        load_source_group(
            src_root,
            HtmlSourceKind::Ui,
            &{
                let mut ui_files = crate::routing::discovery::collect_html_files_pub(
                    &src_root.join("ui"),
                    src_root,
                    &build_config.routing.ignore_directories,
                )?;
                ui_files.sort();
                ui_files
            },
            &src_root.join("ui"),
            &templates_root.join("ui"),
            &mut frag_modules,
            build_config,
        )?;
        load_imported_modules(
            src_root,
            &templates_root.join("imported"),
            &mut frag_modules,
            build_config,
        )?;

        // Expand components and write template files for each fragment module.
        let mut module_paths = frag_modules.keys().cloned().collect::<Vec<_>>();
        module_paths.sort();
        for module_path in module_paths {
            let module = frag_modules
                .get(&module_path)
                .expect("fragment module path exists");
            // Skip ui/ modules — they were already written by the main pipeline.
            if module.kind == HtmlSourceKind::Ui {
                continue;
            }
            let mut stack = vec![module.source_path.clone()];
            let all_modules: HashMap<PathBuf, HtmlModuleSource> = frag_modules.clone();
            let expanded = expand_known_components(
                &module.template_source,
                &module.source_path,
                &all_modules,
                &mut stack,
                0,
            )?;
            let fragment_base = format!("/{url_prefix}");
            let after_islands = transpile_island_tags(&expanded, &fragment_base);
            let action_base = fragment_action_base(&module.source_path, fragment_dir, &url_prefix);
            let (after_react, mut found_react) = transpile_react_tags(
                &after_islands,
                &module.source_path,
                src_root,
                &build_config.client.react,
                &build_config.routing,
                action_base.as_deref(),
            )?;
            react_islands.append(&mut found_react);
            let (after_solid, mut found_solid) = transpile_solid_tags(
                &after_react,
                &module.source_path,
                src_root,
                &build_config.client.solid,
                &build_config.routing,
            )?;
            solid_islands.append(&mut found_solid);
            let after_components = transpile_component_tags(&after_solid);
            let after_pilcrow = transpile_pilcrow_tags(&after_components);
            let final_template = inject_form_method_attrs(&after_pilcrow);

            if let Some(parent) = module.template_output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&module.template_output_path, final_template.as_bytes())?;

            let is_fragment = module.kind == HtmlSourceKind::Fragment;
            files.push(PreprocessedHtmlFile {
                kind: module.kind,
                source_path: module.source_path.clone(),
                template_output_path: module.template_output_path.clone(),
                rust_frontmatter: module.rust_frontmatter.clone(),
                transpiled_template: final_template,
                module_name: module.module_name.clone(),
                render_symbol: module.render_symbol.clone(),
                layout_chain: vec![],
                fragment_url_prefix: if is_fragment {
                    Some(url_prefix.clone())
                } else {
                    None
                },
                layout_chain_ids: vec![],
                page_slot: None,
                ps_head_inner: None,
            });
        }

        // Build route manifest entries for this fragment group.
        let frag_page_routes = build_generated_fragment_manifest(
            src_root,
            fragment_dir,
            &url_prefix,
            &build_config.routing.ignore_directories,
        )?;
        let fragment_error_map = nearest_special_module_for_routes(
            &files,
            &frag_page_routes,
            fragment_dir,
            HtmlSourceKind::ErrorPage,
        );
        let fragment_loading_map = nearest_special_module_for_routes(
            &files,
            &frag_page_routes,
            fragment_dir,
            HtmlSourceKind::LoadingPage,
        );
        fragment_error_module_for_route.extend(fragment_error_map);
        fragment_loading_module_for_route.extend(fragment_loading_map);
        fragment_routes.extend(frag_page_routes);
    }
    // ─────────────────────────────────────────────────────────────────────────

    let generated_routes_file = out_dir.join("generated_routes.rs");
    let generated_routes = write_generated_routes_module(
        src_root,
        &generated_routes_file,
        &build_config.routing.ignore_directories,
        &fragment_dirs,
    )?;
    let generated_templates_file = out_dir.join("generated_templates.rs");
    let manifest_dir = src_root;
    let (react_urls, react_shells) = build_react_assets(
        manifest_dir,
        out_dir,
        &react_islands,
        &build_config.client.react,
    )?;
    let solid_urls = build_solid_assets(
        manifest_dir,
        out_dir,
        &solid_islands,
        &build_config.client.solid,
    )?;
    if !react_urls.is_empty() || !react_shells.is_empty() || !solid_urls.is_empty() {
        for file in &mut files {
            if !react_urls.is_empty() {
                file.transpiled_template =
                    replace_react_placeholders(&file.transpiled_template, &react_urls);
            }
            if !react_shells.is_empty() {
                file.transpiled_template =
                    replace_react_shell_placeholders(&file.transpiled_template, &react_shells);
            }
            if !solid_urls.is_empty() {
                file.transpiled_template =
                    replace_solid_placeholders(&file.transpiled_template, &solid_urls);
            }
            fs::write(
                &file.template_output_path,
                file.transpiled_template.as_bytes(),
            )?;
        }
    }
    let route_params_by_template: HashMap<String, Vec<GeneratedRouteParam>> = generated_routes
        .iter()
        .map(|route| {
            (
                normalize_path_text(Path::new(&route.template_path)),
                route.route_params.clone(),
            )
        })
        .collect();

    let template_codegen_inputs = files
        .iter()
        .map(|file| TemplateCodegenInput {
            module_name: file.module_name.clone(),
            render_symbol: file.render_symbol.clone(),
            source_path: normalize_path_text(&file.source_path),
            rust_frontmatter: file.rust_frontmatter.clone(),
            template_source: file.transpiled_template.clone(),
            layout_chain: file.layout_chain.clone(),
            fragment_url_prefix: file.fragment_url_prefix.clone(),
            route_params: route_params_by_template
                .get(&normalize_path_text(&file.source_path))
                .cloned()
                .unwrap_or_default(),
            layout_chain_ids: file.layout_chain_ids.clone(),
            page_slot: file.page_slot.clone(),
        })
        .collect::<Vec<_>>();
    let templates_output =
        write_generated_templates_module(&template_codegen_inputs, &generated_templates_file)?;
    let generated_templates = templates_output.entries;

    let generated_api_routes_file = out_dir.join("generated_api_routes.rs");
    let generated_api_routes = write_generated_api_routes_module(
        src_root,
        &generated_api_routes_file,
        &build_config.routing.ignore_directories,
    )?;

    // Build directory-keyed maps for special page lookups (nearest-ancestor wins).
    let pages_dir = src_root.join("pages");

    let mut error_module_for_page = nearest_special_module_for_routes(
        &files,
        &generated_routes,
        &pages_dir,
        HtmlSourceKind::ErrorPage,
    );
    error_module_for_page.extend(fragment_error_module_for_route);

    // Root-level _not_found.html is preferred; fall back to the first found.
    let not_found_module: Option<String> = {
        let root_key = normalize_path_text(Path::new(""));
        files
            .iter()
            .filter(|f| f.kind == HtmlSourceKind::NotFoundPage)
            .find(|f| {
                f.source_path
                    .parent()
                    .and_then(|d| d.strip_prefix(&pages_dir).ok())
                    .map(|rel| normalize_path_text(rel) == root_key)
                    .unwrap_or(false)
            })
            .or_else(|| {
                files
                    .iter()
                    .find(|f| f.kind == HtmlSourceKind::NotFoundPage)
            })
            .map(|f| f.module_name.clone())
    };

    let mut loading_module_for_page = nearest_special_module_for_routes(
        &files,
        &generated_routes,
        &pages_dir,
        HtmlSourceKind::LoadingPage,
    );
    loading_module_for_page.extend(fragment_loading_module_for_route);

    // Detect optional hooks.rs and which hooks are defined inside it.
    let hook_flags = detect_hook_flags(&src_root.join("hooks.rs"));

    // Merge page routes and fragment routes for the app module.
    let all_page_routes: Vec<GeneratedPageRoute> = generated_routes
        .iter()
        .cloned()
        .chain(fragment_routes.iter().cloned())
        .collect();

    // Build maps from module symbol → layout chain IDs and page slot pattern.
    // These are derived from the preprocessed files which have the ps data populated.
    let layout_chain_ids_map: HashMap<String, Vec<String>> = files
        .iter()
        .filter(|f| !f.layout_chain_ids.is_empty())
        .map(|f| (f.module_name.clone(), f.layout_chain_ids.clone()))
        .collect();
    let page_slot_map: HashMap<String, String> = files
        .iter()
        .filter_map(|f| f.page_slot.as_ref().map(|s| (f.module_name.clone(), s.clone())))
        .collect();

    // Write the unified app module with auto-wired router.
    write_generated_app_module(
        &all_page_routes,
        &generated_api_routes,
        &AppCodegenMaps {
            load_map: &templates_output.load_map,
            layout_fields_map: &templates_output.layout_fields_map,
            error_module_for_page: &error_module_for_page,
            not_found_module: not_found_module.as_deref(),
            loading_module_for_page: &loading_module_for_page,
            action_map: &templates_output.action_map,
            page_options_map: &templates_output.page_options,
<<<<<<< HEAD
            ssg_config_map: &templates_output.ssg_config_map,
=======
>>>>>>> origin/main
            live_fields_map: &templates_output.live_fields_map,
            has_live_fn_map: &templates_output.has_live_fn_map,
            fsr_live_source_map: &templates_output.fsr_live_source_map,
            fsr_live_fields_map: &templates_output.fsr_live_fields_map,
            layout_chain_ids_map: &layout_chain_ids_map,
            page_slot_map: &page_slot_map,
        },
        hook_flags,
        !react_urls.is_empty(),
        !solid_urls.is_empty(),
        src_root,
        out_dir,
    )?;
    let generated_app_file = out_dir.join("generated_app.rs");

    // Write typed route helpers: `pub mod routes { pub fn index() -> &'static str { "/" } ... }`
    let typed_routes_src =
        crate::templating::routes_codegen::render_generated_typed_routes_module(&all_page_routes);
    fs::write(
        out_dir.join("generated_typed_routes.rs"),
        typed_routes_src.as_bytes(),
    )?;

    // Write env struct helpers: `pub mod env { pub struct Public { ... } pub struct Private { ... } }`
    let env_src = crate::templating::env_codegen::render_generated_env_module(&build_config.env);
    fs::write(out_dir.join("generated_env.rs"), env_src.as_bytes())?;

    // Write i18n typed translation helpers: `pub mod t { pub fn greeting(req, name) -> String }`
    let i18n_src =
        crate::templating::i18n_codegen::render_generated_i18n_module(&build_config.i18n, src_root);
    fs::write(out_dir.join("generated_i18n.rs"), i18n_src.as_bytes())?;

    files.sort_by(|a, b| {
        a.template_output_path
            .cmp(&b.template_output_path)
            .then_with(|| a.source_path.cmp(&b.source_path))
    });

    Ok(CompilerOutput {
        preprocessed_files: files,
        generated_routes_file,
        generated_routes,
        generated_templates_file,
        generated_templates,
        generated_api_routes_file,
        generated_api_routes,
        generated_app_file,
    })
}

/// Compute the URL directory base for a page, used to resolve relative `<island src>` values.
///
/// Strips route-group segments `(name)` from the path. The result is an absolute URL
/// path like `/dashboard` or `/` for root-level pages.
fn page_url_base(source_path: &Path, pages_dir: &Path) -> String {
    let rel = source_path.strip_prefix(pages_dir).unwrap_or(source_path);
    let dir = rel.parent().unwrap_or(Path::new(""));
    let segments: Vec<&str> = dir
        .components()
        .filter_map(|c| {
            if let Component::Normal(s) = c {
                let s = s.to_str()?;
                if s.starts_with('(') && s.ends_with(')') {
                    return None; // strip route groups
                }
                Some(s)
            } else {
                None
            }
        })
        .collect();
    if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", segments.join("/"))
    }
}

fn page_action_base(source_path: &Path, pages_dir: &Path) -> Option<String> {
    let file_path = path_to_unix(source_path);
    let pages_dir = path_to_unix(pages_dir);
    static_action_base(Route::from_path(&file_path, &pages_dir).pattern)
}

fn fragment_action_base(
    source_path: &Path,
    fragment_dir: &Path,
    url_prefix: &str,
) -> Option<String> {
    let file_path = path_to_unix(source_path);
    let fragment_dir = path_to_unix(fragment_dir);
    let mut route = Route::from_path(&file_path, &fragment_dir);
    let clean_prefix = url_prefix.trim_matches('/');
    route.pattern = if route.pattern == "/" {
        format!("/{clean_prefix}")
    } else {
        format!("/{clean_prefix}{}", route.pattern)
    };
    static_action_base(route.pattern)
}

fn static_action_base(pattern: String) -> Option<String> {
    if pattern.contains(':') || pattern.contains('*') {
        None
    } else {
        Some(pattern)
    }
}

fn path_to_unix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Scan `hooks.rs` for known hook function signatures.
///
/// Detection is text-based: we look for `pub async fn <name>(` at the start of a line
/// (after optional leading whitespace). This matches idiomatic `hooks.rs` files without
/// pulling in the full syn parser for a simple presence check.
pub fn detect_hook_flags(hooks_path: &Path) -> HookFlags {
    let src = match fs::read_to_string(hooks_path) {
        Ok(s) => s,
        Err(_) => return HookFlags::default(),
    };
    HookFlags {
        has_handle: src.lines().any(|l| {
            let t = l.trim_start();
            t.starts_with("pub async fn handle(") || t.starts_with("pub async fn handle (")
        }),
        has_handle_error: src.lines().any(|l| {
            let t = l.trim_start();
            t.starts_with("pub async fn handle_error(")
                || t.starts_with("pub async fn handle_error (")
        }),
        has_init: src.lines().any(|l| {
            let t = l.trim_start();
            t.starts_with("pub async fn init(") || t.starts_with("pub async fn init (")
        }),
    }
}

/// Canonical directories and files that should trigger rebuilds in Cargo build scripts.
pub fn watched_source_directories(src_root: impl AsRef<Path>) -> Vec<PathBuf> {
    let src_root = src_root.as_ref();
    vec![
        src_root.join("pages"),
        src_root.join("ui"),
        src_root.join("api"),
        src_root.join("params"),
        src_root.join("hooks.rs"),
    ]
}

fn expand_fragment_entries(
    src_root: &Path,
    build_config: &PilcrowBuildConfig,
) -> io::Result<Vec<FragmentSourceGroup>> {
    let mut groups = Vec::new();

    for entry in &build_config.fragments {
        if has_glob_meta(&entry.dir) {
            let pattern = Glob::new(&entry.dir).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid fragment dir glob `{}`: {err}", entry.dir),
                )
            })?;
            let mut builder = GlobSetBuilder::new();
            builder.add(pattern);
            let set = builder.build().map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid fragment dir glob `{}`: {err}", entry.dir),
                )
            })?;
            let mut dirs = Vec::new();
            collect_matching_dirs(src_root, src_root, &set, &mut dirs)?;
            dirs.sort();
            for dir in dirs {
                let url_prefix = glob_fragment_url_prefix(src_root, &entry.dir, &dir, entry);
                groups.push(FragmentSourceGroup { dir, url_prefix });
            }
        } else {
            let dir = src_root.join(&entry.dir);
            if dir.exists() {
                groups.push(FragmentSourceGroup {
                    dir,
                    url_prefix: entry.url_prefix(),
                });
            }
        }
    }

    groups.sort_by(|a, b| {
        a.dir
            .cmp(&b.dir)
            .then_with(|| a.url_prefix.cmp(&b.url_prefix))
    });
    groups.dedup_by(|a, b| a.dir == b.dir && a.url_prefix == b.url_prefix);
    Ok(groups)
}

fn has_glob_meta(value: &str) -> bool {
    value.contains('*') || value.contains('?') || value.contains('[')
}

fn glob_fragment_url_prefix(
    src_root: &Path,
    pattern: &str,
    dir: &Path,
    entry: &crate::templating::build_config::FragmentEntry,
) -> String {
    let base_url = entry.url.clone().unwrap_or_else(|| {
        dir.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("fragments")
            .to_string()
    });
    let rel = dir
        .strip_prefix(src_root)
        .unwrap_or(dir)
        .to_string_lossy()
        .replace('\\', "/");
    let rel_segments = rel
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let pattern_segments = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    let first_glob = pattern_segments
        .iter()
        .position(|segment| has_glob_meta(segment))
        .unwrap_or(pattern_segments.len());
    let last_glob = pattern_segments
        .iter()
        .rposition(|segment| has_glob_meta(segment))
        .unwrap_or(first_glob);
    let fixed_prefix_len = first_glob;
    let fixed_suffix_len = pattern_segments
        .len()
        .saturating_sub(last_glob.saturating_add(1));
    let capture_end = rel_segments.len().saturating_sub(fixed_suffix_len);
    let capture = if fixed_prefix_len <= capture_end {
        rel_segments[fixed_prefix_len..capture_end].join("/")
    } else {
        String::new()
    };

    if capture.is_empty() {
        base_url
    } else {
        format!(
            "{}/{}",
            base_url.trim_matches('/'),
            capture.trim_matches('/')
        )
    }
}

fn collect_matching_dirs(
    dir: &Path,
    base: &Path,
    set: &globset::GlobSet,
    out: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let rel = path.strip_prefix(base).unwrap_or(&path);
        if set.is_match(rel) {
            out.push(path.clone());
        }
        collect_matching_dirs(&path, base, set, out)?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct HtmlModuleSource {
    kind: HtmlSourceKind,
    source_path: PathBuf,
    template_output_path: PathBuf,
    rust_frontmatter: String,
    template_source: String,
    module_name: String,
    render_symbol: String,
    imports: HashMap<String, PathBuf>,
    /// Ordered layout chain for this page: [outermost_auto_layout, ..., explicit_layout].
    /// Populated during `preprocess_discovered_sources` after all modules are loaded.
    layout_chain: Vec<String>,
    /// When true (from `pub const LAYOUT: &str = "none"`), skip all auto-layout wrapping.
    skip_layout: bool,
    /// Layout IDs for each auto-layout in the chain (outermost first).
    layout_chain_ids: Vec<String>,
    /// Route pattern used as the data-ps-slot value for this page.
    page_slot: Option<String>,
    /// Inner content of `<pilcrow:head>` from the page template, extracted before
    /// the pipeline strips it. Used to emit `<template data-ps-head>` in the output.
    ps_head_inner: Option<String>,
}

fn preprocess_discovered_sources(
    src_root: &Path,
    templates_root: &Path,
    discovered: &DiscoveredHtmlFiles,
    build_config: &PilcrowBuildConfig,
    react_islands: &mut Vec<ReactIslandRef>,
    solid_islands: &mut Vec<SolidIslandRef>,
) -> io::Result<Vec<PreprocessedHtmlFile>> {
    let mut modules = HashMap::<PathBuf, HtmlModuleSource>::new();

    // Auto-layouts live inside pages/ but are treated as layout modules.
    // Load them first so their module names are available when building page chains.
    let pages_dir = src_root.join("pages");
    load_source_group(
        src_root,
        HtmlSourceKind::AutoLayout,
        &discovered.auto_layouts,
        &pages_dir,
        &templates_root.join("auto_layouts"),
        &mut modules,
        build_config,
    )?;
    // Error boundaries, not-found, and loading pages are also in pages/ but not routable.
    load_source_group(
        src_root,
        HtmlSourceKind::ErrorPage,
        &discovered.error_pages,
        &pages_dir,
        &templates_root.join("error_pages"),
        &mut modules,
        build_config,
    )?;
    load_source_group(
        src_root,
        HtmlSourceKind::NotFoundPage,
        &discovered.not_found_pages,
        &pages_dir,
        &templates_root.join("not_found_pages"),
        &mut modules,
        build_config,
    )?;
    load_source_group(
        src_root,
        HtmlSourceKind::LoadingPage,
        &discovered.loading_pages,
        &pages_dir,
        &templates_root.join("loading_pages"),
        &mut modules,
        build_config,
    )?;
    load_source_group(
        src_root,
        HtmlSourceKind::Page,
        &discovered.pages,
        &pages_dir,
        &templates_root.join("pages"),
        &mut modules,
        build_config,
    )?;
    load_source_group(
        src_root,
        HtmlSourceKind::Ui,
        &discovered.ui,
        &src_root.join("ui"),
        &templates_root.join("ui"),
        &mut modules,
        build_config,
    )?;
    load_imported_modules(
        src_root,
        &templates_root.join("imported"),
        &mut modules,
        build_config,
    )?;

    // Now that all modules are loaded, inject auto-layout wrapping for each page.
    // Walk up the page's directory to find _layout.html files; wrap the template.
    let page_paths: Vec<PathBuf> = modules
        .keys()
        .filter(|p| {
            modules
                .get(*p)
                .is_some_and(|m| m.kind == HtmlSourceKind::Page)
        })
        .cloned()
        .collect();

    for page_path in &page_paths {
        // Skip layout wrapping entirely when the page opts out via `pub const LAYOUT: &str = "none"`.
        if modules.get(page_path).is_some_and(|m| m.skip_layout) {
            continue;
        }

        let auto_chain = build_auto_layout_chain(page_path, &pages_dir, &modules);
        if auto_chain.is_empty() {
            continue;
        }

        // Compute layout chain IDs and page slot pattern for data-ps-* attributes.
        let layout_ids: Vec<String> = auto_chain
            .iter()
            .map(|(lp, _)| layout_id_from_path(lp, &pages_dir))
            .collect();
        let slot_pat = page_slot_pattern(page_path, &pages_dir);

        // Inject synthetic imports and wrap the template.
        {
            let page_module = modules.get_mut(page_path).expect("page path exists");
            let original = page_module.template_source.clone();

            // Extract pilcrow:head inner content for the data-ps-head template injection.
            // This is extracted now because <pilcrow:head> is stripped by transpile_pilcrow_tags
            // and will not be visible after the expansion pipeline.
            let head_inner = extract_pilcrow_head_inner(&original).unwrap_or_default();

            // Add synthetic import aliases: PilcrowAutoLayout0 (outermost), PilcrowAutoLayout1, ...
            for (i, (layout_path, _)) in auto_chain.iter().enumerate() {
                let alias = format!("PilcrowAutoLayout{i}");
                page_module.imports.insert(alias, layout_path.clone());
            }

            // Wrap template from innermost to outermost.
            // NOTE: data-ps-* wrapper elements are injected as a POST-PROCESSING step AFTER
            // expand_known_components completes (see inject_ps_nav_markers below). We use
            // plain-text sentinel strings here — they become part of the default slot content
            // and are passed through by collect_slot_assignments unchanged, while named-slot
            // elements (slot="header") remain visible as direct children of the invocation.
            //
            // Sentinels:
            //   __PS_SLOT_OPEN__<slot_pat>__   — marks where <div data-ps-slot> starts
            //   __PS_SLOT_CLOSE__               — marks where <div data-ps-slot> ends
            //   __PS_LAYOUT_OPEN__<layout_id>__ — marks where <div data-ps-layout> starts
            //   __PS_LAYOUT_CLOSE__             — marks where <div data-ps-layout> ends
            // head_inner is stored separately in HtmlModuleSource.ps_head_inner.
            let mut wrapped = format!(
                "__PS_SLOT_OPEN__{slot_pat}__\
                 {original}\
                 __PS_SLOT_CLOSE__"
            );
            for i in (0..auto_chain.len()).rev() {
                let alias = format!("PilcrowAutoLayout{i}");
                let layout_id = &layout_ids[i];
                wrapped = format!(
                    "<{alias}>__PS_LAYOUT_OPEN__{layout_id}__\
                     {wrapped}\
                     __PS_LAYOUT_CLOSE__</{alias}>"
                );
            }
            page_module.template_source = wrapped;

            // Store layout chain ids, page slot, and extracted head content on the module.
            page_module.layout_chain_ids = layout_ids.clone();
            page_module.page_slot = Some(slot_pat.clone());
            page_module.ps_head_inner = Some(head_inner);
        }

        // Prepend auto-layout module names to the layout_chain (outermost first).
        {
            let page_module = modules.get_mut(page_path).expect("page path exists");
            let auto_module_names: Vec<String> =
                auto_chain.iter().map(|(_, name)| name.clone()).collect();
            let explicit: Vec<String> = page_module.layout_chain.drain(..).collect();
            page_module.layout_chain = auto_module_names;
            page_module.layout_chain.extend(explicit);
        }
    }

    let mut module_paths = modules.keys().cloned().collect::<Vec<_>>();
    module_paths.sort();

    let mut out = Vec::new();
    for module_path in module_paths {
        let module = modules
            .get(&module_path)
            .expect("module path from keys should exist");

        let mut stack = vec![module.source_path.clone()];
        let expanded_raw = expand_known_components(
            &module.template_source,
            &module.source_path,
            &modules,
            &mut stack,
            0,
        )?;
        // Inject PS nav markers into the expanded template (only for pages with auto-layout).
        let expanded = if module.kind == HtmlSourceKind::Page && !module.layout_chain_ids.is_empty()
        {
            inject_ps_nav_markers(&expanded_raw, module.ps_head_inner.as_deref())
        } else {
            expanded_raw
        };
        let url_base = if module.kind == HtmlSourceKind::Page {
            page_url_base(&module.source_path, &pages_dir)
        } else {
            "/".to_string()
        };
        let after_islands = transpile_island_tags(&expanded, &url_base);
        let action_base = if module.kind == HtmlSourceKind::Page {
            page_action_base(&module.source_path, &pages_dir)
        } else {
            None
        };
        let (after_react, mut found_react) = transpile_react_tags(
            &after_islands,
            &module.source_path,
            src_root,
            &build_config.client.react,
            &build_config.routing,
            action_base.as_deref(),
        )?;
        react_islands.append(&mut found_react);
        let (after_solid, mut found_solid) = transpile_solid_tags(
            &after_react,
            &module.source_path,
            src_root,
            &build_config.client.solid,
            &build_config.routing,
        )?;
        solid_islands.append(&mut found_solid);
        let after_components = transpile_component_tags(&after_solid);
        let after_pilcrow = transpile_pilcrow_tags(&after_components);
        let final_template = inject_form_method_attrs(&after_pilcrow);

        if let Some(parent) = module.template_output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&module.template_output_path, final_template.as_bytes())?;

        out.push(PreprocessedHtmlFile {
            kind: module.kind,
            source_path: module.source_path.clone(),
            template_output_path: module.template_output_path.clone(),
            rust_frontmatter: module.rust_frontmatter.clone(),
            transpiled_template: final_template,
            module_name: module.module_name.clone(),
            render_symbol: module.render_symbol.clone(),
            layout_chain: module.layout_chain.clone(),
            fragment_url_prefix: None,
            layout_chain_ids: module.layout_chain_ids.clone(),
            page_slot: module.page_slot.clone(),
            ps_head_inner: module.ps_head_inner.clone(),
        });
    }

    Ok(out)
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("mdx"))
}

fn load_source_group(
    src_root: &Path,
    kind: HtmlSourceKind,
    source_files: &[PathBuf],
    source_root: &Path,
    out_root: &Path,
    modules: &mut HashMap<PathBuf, HtmlModuleSource>,
    build_config: &PilcrowBuildConfig,
) -> io::Result<()> {
    for source_path in source_files {
        let source = fs::read_to_string(source_path)?;
        let parts = if is_markdown_path(source_path) {
            transpile_markdown(&source).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "failed to compile markdown {}: {err}",
                        source_path.display()
                    ),
                )
            })?
        } else {
            split_html_module(&source).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to parse {}: {err}", source_path.display()),
                )
            })?
        };

        // Error and not-found pages have framework-injected Props.
        // Their frontmatter (if any) may only contain `import` statements.
        let (cleaned_frontmatter, imports) = if matches!(
            kind,
            HtmlSourceKind::ErrorPage | HtmlSourceKind::NotFoundPage | HtmlSourceKind::LoadingPage
        ) {
            let (leftover, imports) =
                strip_frontmatter_imports(&parts.rust, src_root, source_path, build_config)?;
            if !leftover.trim().is_empty() {
                return Err(template_compile_error(
                    source_path,
                    "frontmatter in this special page may only contain `import` \
                     statements — Props are provided by the framework",
                ));
            }
            let injected_props = match kind {
                HtmlSourceKind::ErrorPage => {
                    "pub struct Props {\n    pub status: u16,\n    pub message: String,\n}\n"
                }
                HtmlSourceKind::NotFoundPage | HtmlSourceKind::LoadingPage => {
                    "pub struct Props {}\n"
                }
                _ => unreachable!(),
            };
            (injected_props.to_string(), imports)
        } else {
            // Code-behind: if a sibling `.rs` file exists, it carries all Rust logic.
            // The `---` block in the `.html` file must then contain only `import` statements.
            let codebehind_path = source_path.with_extension("rs");
            if codebehind_path.exists() {
                let (leftover, imports) =
                    strip_frontmatter_imports(&parts.rust, src_root, source_path, build_config)?;
                if !leftover.trim().is_empty() {
                    return Err(template_compile_error(
                        source_path,
                        "frontmatter may only contain `import` statements when a \
                         code-behind `.rs` file is present — move Rust logic there",
                    ));
                }
                let rs_content = fs::read_to_string(&codebehind_path).map_err(|err| {
                    template_compile_error(
                        source_path,
                        format!(
                            "failed to read code-behind `{}`: {err}",
                            codebehind_path.display()
                        ),
                    )
                })?;
                (rs_content, imports)
            } else {
                strip_frontmatter_imports(&parts.rust, src_root, source_path, build_config)?
            }
        };

        let relative = source_path.strip_prefix(source_root).unwrap_or(source_path);
        let template_output_path = if is_markdown_path(source_path) {
            out_root.join(relative.with_extension("html"))
        } else {
            out_root.join(relative)
        };
        let module_name = build_module_name(kind, relative);
        let render_symbol = format!("render_{module_name}");

        // layout_chain starts empty; auto-layout entries are prepended in
        // `preprocess_discovered_sources` after all modules are loaded.
        let layout_chain: Vec<String> = vec![];
        let skip_layout = kind == HtmlSourceKind::Page && scan_layout_none(&cleaned_frontmatter);

        modules.insert(
            source_path.clone(),
            HtmlModuleSource {
                kind,
                source_path: source_path.clone(),
                template_output_path,
                rust_frontmatter: cleaned_frontmatter,
                template_source: parts.template,
                module_name,
                render_symbol,
                imports,
                layout_chain,
                skip_layout,
                layout_chain_ids: vec![],
                page_slot: None,
                ps_head_inner: None,
            },
        );
    }

    Ok(())
}

/// Load fragment HTML files into the module graph.
///
/// Fragments behave like pages but have no layout auto-wrapping (empty `layout_chain`).
/// The module name is `frag_{url_prefix}_{snake_relative}`.
fn load_fragment_source_group(
    src_root: &Path,
    url_prefix: &str,
    source_files: &[PathBuf],
    fragment_dir: &Path,
    out_root: &Path,
    modules: &mut HashMap<PathBuf, HtmlModuleSource>,
    build_config: &PilcrowBuildConfig,
) -> io::Result<()> {
    for source_path in source_files {
        let source = fs::read_to_string(source_path)?;
        let parts = if is_markdown_path(source_path) {
            transpile_markdown(&source).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "failed to compile markdown {}: {err}",
                        source_path.display()
                    ),
                )
            })?
        } else {
            split_html_module(&source).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to parse {}: {err}", source_path.display()),
                )
            })?
        };

        let codebehind_path = source_path.with_extension("rs");
        let (cleaned_frontmatter, imports) = if codebehind_path.exists() {
            let (leftover, imports) =
                strip_frontmatter_imports(&parts.rust, src_root, source_path, build_config)?;
            if !leftover.trim().is_empty() {
                return Err(template_compile_error(
                    source_path,
                    "frontmatter may only contain `import` statements when a \
                     code-behind `.rs` file is present — move Rust logic there",
                ));
            }
            let rs_content = fs::read_to_string(&codebehind_path).map_err(|err| {
                template_compile_error(
                    source_path,
                    format!(
                        "failed to read code-behind `{}`: {err}",
                        codebehind_path.display()
                    ),
                )
            })?;
            (rs_content, imports)
        } else {
            strip_frontmatter_imports(&parts.rust, src_root, source_path, build_config)?
        };

        let relative_in_dir = source_path
            .strip_prefix(fragment_dir)
            .unwrap_or(source_path);
        let template_output_path = if is_markdown_path(source_path) {
            out_root.join(relative_in_dir.with_extension("html"))
        } else {
            out_root.join(relative_in_dir)
        };
        // Module name: `frag_{url_prefix}_{snake_relative}` — pass prefixed path to build_module_name.
        let prefixed_relative = Path::new(url_prefix).join(relative_in_dir);
        let module_name = build_module_name(HtmlSourceKind::Fragment, &prefixed_relative);
        let render_symbol = format!("render_{module_name}");

        modules.insert(
            source_path.clone(),
            HtmlModuleSource {
                kind: HtmlSourceKind::Fragment,
                source_path: source_path.clone(),
                template_output_path,
                rust_frontmatter: cleaned_frontmatter,
                template_source: parts.template,
                module_name,
                render_symbol,
                imports,
                layout_chain: vec![],
                skip_layout: false,
                layout_chain_ids: vec![],
                page_slot: None,
                ps_head_inner: None,
            },
        );
    }
    Ok(())
}

fn load_imported_modules(
    src_root: &Path,
    out_root: &Path,
    modules: &mut HashMap<PathBuf, HtmlModuleSource>,
    build_config: &PilcrowBuildConfig,
) -> io::Result<()> {
    loop {
        let mut missing = modules
            .values()
            .flat_map(|module| module.imports.values())
            .filter(|path| !modules.contains_key(*path))
            .cloned()
            .collect::<Vec<_>>();
        missing.sort();
        missing.dedup();

        if missing.is_empty() {
            break;
        }

        load_source_group(
            src_root,
            HtmlSourceKind::Ui,
            &missing,
            src_root,
            out_root,
            modules,
            build_config,
        )?;
    }

    Ok(())
}

/// Replace PS navigation sentinel strings with real HTML elements.
///
/// The sentinel strings were injected into the template source BEFORE
/// `expand_known_components` so that they survive the slot distribution system
/// (plain-text sentinels are treated as opaque default-slot content).
///
/// After expansion:
/// - `__PS_LAYOUT_OPEN__<id>__...__PS_LAYOUT_CLOSE__` → `<div data-ps-layout="<id>">...</div>`
/// - `__PS_SLOT_OPEN__<pat>__...__PS_SLOT_CLOSE__`     → `<div data-ps-slot="<pat>">...</div>`
///
/// The `<template data-ps-head>` is prepended using `head_inner` which was extracted
/// before the pipeline stripped `<pilcrow:head>` blocks.
fn inject_ps_nav_markers(template: &str, head_inner: Option<&str>) -> String {
    let mut out = template.to_string();

    // Replace nested layout markers: __PS_LAYOUT_OPEN__<id>__...__PS_LAYOUT_CLOSE__
    // Process iteratively until no more remain (supports multiple nesting levels).
    for _ in 0..16 {
        const LO: &str = "__PS_LAYOUT_OPEN__";
        const LS: &str = "__";
        const LC: &str = "__PS_LAYOUT_CLOSE__";
        let Some(lo_pos) = out.rfind(LO) else { break };
        let id_start = lo_pos + LO.len();
        let Some(sep_off) = out[id_start..].find(LS) else { break };
        let id = out[id_start..id_start + sep_off].to_string();
        let content_start = id_start + sep_off + LS.len();
        let Some(lc_off) = out[content_start..].find(LC) else { break };
        let content = &out[content_start..content_start + lc_off];
        let replacement = format!("<div data-ps-layout=\"{id}\">{content}</div>");
        let end = content_start + lc_off + LC.len();
        out.replace_range(lo_pos..end, &replacement);
    }

    // Replace slot marker: __PS_SLOT_OPEN__<pat>__...__PS_SLOT_CLOSE__
    {
        const SO: &str = "__PS_SLOT_OPEN__";
        const SS: &str = "__"; // separator after pattern
        const SC: &str = "__PS_SLOT_CLOSE__";
        if let Some(so_pos) = out.find(SO) {
            let pat_start = so_pos + SO.len();
            if let Some(sep_off) = out[pat_start..].find(SS) {
                let pat = out[pat_start..pat_start + sep_off].to_string();
                let content_start = pat_start + sep_off + SS.len();
                if let Some(sc_off) = out[content_start..].find(SC) {
                    let content = out[content_start..content_start + sc_off].to_string();
                    let head_tpl = match head_inner {
                        Some(h) if !h.is_empty() => {
                            format!("<template data-ps-head>{h}</template>")
                        }
                        _ => String::new(),
                    };
                    let replacement =
                        format!("{head_tpl}<div data-ps-slot=\"{pat}\">{content}</div>");
                    out = format!(
                        "{}{}{}",
                        &out[..so_pos],
                        replacement,
                        &out[content_start + sc_off + SC.len()..]
                    );
                }
            }
        }
    }

    out
}

/// Derive a stable layout identifier from its file path.
/// `pages/_layout.html` → `"/"`
/// `pages/tickets/_layout.html` → `"/tickets"`
fn layout_id_from_path(layout_path: &Path, pages_dir: &Path) -> String {
    let dir = layout_path.parent().unwrap_or(pages_dir);
    let rel = dir.strip_prefix(pages_dir).unwrap_or(Path::new(""));
    let s = rel.to_string_lossy().replace('\\', "/");
    if s.is_empty() {
        "/".to_string()
    } else {
        format!("/{s}")
    }
}

/// Derive the route pattern for a page file (same logic as the router).
fn page_slot_pattern(page_path: &Path, pages_dir: &Path) -> String {
    let file_path = path_to_unix(page_path);
    let pages_dir_str = path_to_unix(pages_dir);
    let route = crate::Route::from_path(&file_path, &pages_dir_str);
    route.pattern
}

/// Extract the inner text of the first `<pilcrow:head>…</pilcrow:head>` block.
fn extract_pilcrow_head_inner(template: &str) -> Option<String> {
    const OPEN_PREFIX: &str = "<pilcrow:head";
    let start = template.find(OPEN_PREFIX)?;
    let after_prefix = &template[start + OPEN_PREFIX.len()..];
    let gt = after_prefix.find('>')?;
    let inner_start = start + OPEN_PREFIX.len() + gt + 1;
    const CLOSE: &str = "</pilcrow:head>";
    let close_off = template[inner_start..].find(CLOSE)?;
    Some(template[inner_start..inner_start + close_off].to_string())
}

/// Walk up from a page's directory (within `pages_dir`) and collect any
/// `_layout.html` auto-layout files found along the way.
///
/// Returns `(absolute_path, module_name)` pairs ordered **outermost first**
/// (i.e. root `_layout.html` before subdirectory-level ones).
fn build_auto_layout_chain(
    page_path: &Path,
    pages_dir: &Path,
    modules: &HashMap<PathBuf, HtmlModuleSource>,
) -> Vec<(PathBuf, String)> {
    let relative = page_path.strip_prefix(pages_dir).unwrap_or(page_path);
    let mut dir = relative.parent().unwrap_or(Path::new(""));
    let mut chain: Vec<(PathBuf, String)> = Vec::new();

    loop {
        let layout_rel = dir.join("_layout.html");
        let layout_abs = pages_dir.join(&layout_rel);
        if modules.contains_key(&layout_abs) {
            let module_name = build_module_name(HtmlSourceKind::AutoLayout, &layout_rel);
            chain.push((layout_abs, module_name));
        }
        if dir.as_os_str().is_empty() {
            break;
        }
        dir = dir.parent().unwrap_or(Path::new(""));
    }

    // Collected innermost-first; reverse to get outermost-first.
    chain.reverse();
    chain
}

fn nearest_special_module_for_routes(
    files: &[PreprocessedHtmlFile],
    routes: &[GeneratedPageRoute],
    root_dir: &Path,
    kind: HtmlSourceKind,
) -> HashMap<String, String> {
    let dir_map: HashMap<String, String> = files
        .iter()
        .filter(|file| file.kind == kind)
        .filter_map(|file| {
            let dir = file.source_path.parent()?;
            let dir_rel = dir.strip_prefix(root_dir).ok()?;
            Some((normalize_path_text(dir_rel), file.module_name.clone()))
        })
        .collect();

    routes
        .iter()
        .filter_map(|route| {
            let abs_path = PathBuf::from(&route.template_path);
            let page_dir = abs_path.parent()?;
            let mut dir = page_dir.strip_prefix(root_dir).ok()?;

            loop {
                let key = normalize_path_text(dir);
                if let Some(module) = dir_map.get(&key) {
                    return Some((route.symbol.clone(), module.clone()));
                }
                if dir.as_os_str().is_empty() {
                    break;
                }
                dir = dir.parent().unwrap_or(Path::new(""));
            }
            None
        })
        .collect()
}

fn build_module_name(kind: HtmlSourceKind, relative: &Path) -> String {
    // AutoLayout, ErrorPage, and NotFoundPage are all named by their parent directory,
    // not by filename. Extract a helper closure for that logic.
    let dir_based_name = |prefix: &str, fallback: &str| -> String {
        let parent = relative.parent().unwrap_or(Path::new(""));
        let dir_str = parent.to_string_lossy().replace('\\', "/");
        let dir_part = dir_str.trim_matches('/');
        if dir_part.is_empty() {
            return format!("{prefix}_{fallback}");
        }
        // Strip layout-group segments and normalise to a symbol.
        let stripped: String = dir_part
            .split('/')
            .filter(|seg| !(seg.starts_with('(') && seg.ends_with(')')))
            .collect::<Vec<_>>()
            .join("/");
        let mut symbol = String::new();
        let mut prev_under = false;
        for ch in stripped.chars() {
            let mapped = if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            };
            if mapped == '_' {
                if !prev_under {
                    symbol.push('_');
                }
                prev_under = true;
            } else {
                symbol.push(mapped);
                prev_under = false;
            }
        }
        let symbol = symbol.trim_matches('_').to_string();
        if symbol.is_empty() {
            // All segments were stripped (pure route-group dir like `(admin)`).
            // Use the raw dir to build a unique suffix so two groups don't collide.
            let raw_symbol: String = dir_part
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
                .trim_matches('_')
                .to_string();
            let raw_symbol = raw_symbol.trim_matches('_');
            if raw_symbol.is_empty() {
                format!("{prefix}_{fallback}")
            } else {
                format!("{prefix}_{raw_symbol}")
            }
        } else {
            format!("{prefix}_{symbol}")
        }
    };

    let prefix = match kind {
        HtmlSourceKind::Page => "page",
        HtmlSourceKind::Ui => "ui",
        HtmlSourceKind::Fragment => {
            // `relative` is already prefixed with the url_prefix, e.g. `widgets/user-card.html`.
            let relative = relative.to_string_lossy().replace('\\', "/");
            let without_ext = relative
                .strip_suffix(".html")
                .or_else(|| relative.strip_suffix(".md"))
                .or_else(|| relative.strip_suffix(".mdx"))
                .unwrap_or(&relative);
            let mut symbol = String::new();
            let mut prev_under = false;
            for ch in without_ext.chars() {
                let mapped = if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '_'
                };
                if mapped == '_' {
                    if !prev_under {
                        symbol.push('_');
                    }
                    prev_under = true;
                } else {
                    symbol.push(mapped);
                    prev_under = false;
                }
            }
            let s = symbol.trim_matches('_');
            let s = if s.is_empty() { "index" } else { s };
            return format!("frag_{s}");
        }
        HtmlSourceKind::AutoLayout => {
            // `products/_layout.html` → `layout_auto_products`
            // `_layout.html`         → `layout_auto_root`
            return dir_based_name("layout_auto", "root");
        }
        HtmlSourceKind::ErrorPage => {
            // `products/_error.html` → `error_products`
            // `_error.html`         → `error_root`
            return dir_based_name("error", "root");
        }
        HtmlSourceKind::NotFoundPage => {
            // `products/_not_found.html` → `not_found_products`
            // `_not_found.html`          → `not_found_root`
            return dir_based_name("not_found", "root");
        }
        HtmlSourceKind::LoadingPage => {
            // `products/_loading.html` → `loading_products`
            // `_loading.html`          → `loading_root`
            return dir_based_name("loading", "root");
        }
    };

    let relative = relative.to_string_lossy().replace('\\', "/");
    let without_ext = relative
        .strip_suffix(".html")
        .or_else(|| relative.strip_suffix(".md"))
        .or_else(|| relative.strip_suffix(".mdx"))
        .unwrap_or(&relative);

    // Strip layout-group segments `(group)` so they don't pollute module names.
    // e.g. `(app)/settings/profile` → `settings/profile`
    let stripped: String = without_ext
        .split('/')
        .filter(|seg| !(seg.starts_with('(') && seg.ends_with(')')))
        .collect::<Vec<_>>()
        .join("/");

    let mut symbol = String::new();
    let mut prev_underscore = false;
    for ch in stripped.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };

        if mapped == '_' {
            if !prev_underscore {
                symbol.push('_');
            }
            prev_underscore = true;
        } else {
            symbol.push(mapped);
            prev_underscore = false;
        }
    }

    let symbol = symbol.trim_matches('_');
    if symbol.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}_{symbol}")
    }
}

fn normalize_path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn src_relative_display_path(path: &Path) -> String {
    let normalized = normalize_path_text(path);
    if let Some((_, tail)) = normalized.split_once("/src/") {
        return tail.to_string();
    }
    if let Some(tail) = normalized.strip_prefix("src/") {
        return tail.to_string();
    }
    normalized
}

/// Quick string scan — returns true if the source contains `pub const LAYOUT` set to `"none"`.
/// Used before full syn parsing so we can set `skip_layout` on the module source early.
fn scan_layout_none(source: &str) -> bool {
    source.contains("pub const LAYOUT") && {
        // Find the value after the `=` on the same statement.
        if let Some(pos) = source.find("pub const LAYOUT") {
            let rest = &source[pos..];
            if let Some(eq) = rest.find('=') {
                let value_part = rest[eq + 1..].trim_start();
                value_part.starts_with("\"none\"") || value_part.starts_with("'none'")
            } else {
                false
            }
        } else {
            false
        }
    }
}

fn template_compile_error(source_path: &Path, message: impl Into<String>) -> io::Error {
    let file = src_relative_display_path(source_path);
    let message = message.into();
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("file: {file}\nerror: {message}"),
    )
}

fn line_col_at(text: &str, byte_idx: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;

    for (idx, ch) in text.char_indices() {
        if idx >= byte_idx {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }

    (line, col)
}

fn suggested_import_paths(
    alias: &str,
    modules: &HashMap<PathBuf, HtmlModuleSource>,
) -> Vec<String> {
    let mut paths = modules
        .values()
        .filter_map(|module| {
            let stem = module.source_path.file_stem().and_then(|s| s.to_str())?;
            if stem != alias {
                return None;
            }
            let rel = src_relative_display_path(&module.source_path);
            if rel.starts_with("ui/") {
                Some(rel)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn strip_frontmatter_imports(
    rust_frontmatter: &str,
    src_root: &Path,
    source_path: &Path,
    build_config: &PilcrowBuildConfig,
) -> io::Result<(String, HashMap<String, PathBuf>)> {
    let mut imports = HashMap::<String, PathBuf>::new();
    let mut kept_lines = Vec::new();

    for line in rust_frontmatter.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("import ") {
            let (alias, import_path) = parse_import_statement(trimmed, source_path)?;
            if !is_pascal_case_name(&alias) {
                return Err(template_compile_error(
                    source_path,
                    format!("invalid import alias `{alias}`; expected PascalCase alias"),
                ));
            }

            let resolved = resolve_import_path(src_root, source_path, &import_path, build_config)?;
            if imports.insert(alias.clone(), resolved).is_some() {
                return Err(template_compile_error(
                    source_path,
                    format!("duplicate import alias `{alias}`"),
                ));
            }
            continue;
        }

        kept_lines.push(line.to_string());
    }

    Ok((kept_lines.join("\n").trim().to_string(), imports))
}

fn parse_import_statement(line: &str, source_path: &Path) -> io::Result<(String, String)> {
    if !line.ends_with(';') {
        return Err(template_compile_error(
            source_path,
            format!("invalid import syntax `{line}` (expected trailing `;`)"),
        ));
    }

    let no_semicolon = line[..line.len() - 1].trim_end();
    let rest = no_semicolon.strip_prefix("import ").ok_or_else(|| {
        template_compile_error(source_path, format!("invalid import syntax `{line}`"))
    })?;

    let (alias_raw, from_raw) = rest.split_once(" from ").ok_or_else(|| {
        template_compile_error(source_path, format!("invalid import syntax `{line}`"))
    })?;

    let alias = alias_raw.trim().to_string();
    if alias.is_empty() {
        return Err(template_compile_error(
            source_path,
            format!("invalid import syntax `{line}` (missing alias)"),
        ));
    }

    let import_path = parse_quoted_literal(from_raw.trim(), source_path, line)?;
    Ok((alias, import_path))
}

fn parse_quoted_literal(value: &str, source_path: &Path, line: &str) -> io::Result<String> {
    if value.len() < 2 {
        return Err(template_compile_error(
            source_path,
            format!("invalid import syntax `{line}` (expected quoted path)"),
        ));
    }

    let bytes = value.as_bytes();
    let quote = bytes[0];
    let valid_quote = quote == b'"' || quote == b'\'';
    if !valid_quote || bytes[value.len() - 1] != quote {
        return Err(template_compile_error(
            source_path,
            format!("invalid import syntax `{line}` (expected quoted path)"),
        ));
    }

    Ok(value[1..value.len() - 1].to_string())
}

fn resolve_import_path(
    src_root: &Path,
    source_path: &Path,
    import_path: &str,
    build_config: &PilcrowBuildConfig,
) -> io::Result<PathBuf> {
    let normalized = import_path.replace('\\', "/");
    if normalized.is_empty() || normalized.starts_with('/') {
        return Err(template_compile_error(
            source_path,
            format!("invalid import path `{normalized}`; expected relative path or import alias"),
        ));
    }
    if !normalized.ends_with(".html") {
        return Err(template_compile_error(
            source_path,
            format!("invalid import path `{normalized}`; expected `.html` import"),
        ));
    }
    let absolute = if normalized.starts_with("./") || normalized.starts_with("../") {
        source_path
            .parent()
            .unwrap_or(src_root)
            .join(Path::new(&normalized))
    } else {
        let (alias, rest) = normalized.split_once('/').ok_or_else(|| {
            template_compile_error(
                source_path,
                format!("invalid import path `{normalized}`; expected `<alias>/...`"),
            )
        })?;
        let default_ui = "ui".to_string();
        let alias_root = build_config
            .imports
            .get(alias)
            .or_else(|| (alias == "ui").then_some(&default_ui))
            .ok_or_else(|| {
                template_compile_error(
                    source_path,
                    format!("unknown import alias `{alias}` in `{normalized}`"),
                )
            })?;
        if alias_root.is_empty() || alias_root.starts_with('/') {
            return Err(template_compile_error(
                source_path,
                format!("invalid import alias `{alias}` target `{alias_root}`"),
            ));
        }
        src_root.join(alias_root).join(rest)
    };

    let canonical_root = src_root.canonicalize().map_err(|err| {
        template_compile_error(
            source_path,
            format!(
                "failed to resolve project root `{}`: {err}",
                src_root.display()
            ),
        )
    })?;
    let canonical_absolute = absolute.canonicalize().map_err(|_| {
        template_compile_error(
            source_path,
            format!("import path `{normalized}` was not found on disk"),
        )
    })?;

    if !canonical_absolute.starts_with(&canonical_root) {
        return Err(template_compile_error(
            source_path,
            format!("import path `{normalized}` escapes the project root"),
        ));
    }

    Ok(absolute)
}

fn expand_known_components(
    template: &str,
    owner_path: &Path,
    modules: &HashMap<PathBuf, HtmlModuleSource>,
    stack: &mut Vec<PathBuf>,
    depth: usize,
) -> io::Result<String> {
    const MAX_DEPTH: usize = 64;
    if depth >= MAX_DEPTH {
        return Err(template_compile_error(
            owner_path,
            format!("component expansion exceeded max depth ({MAX_DEPTH})"),
        ));
    }

    let owner_module = modules.get(owner_path).ok_or_else(|| {
        template_compile_error(
            owner_path,
            "template source was not registered in module graph",
        )
    })?;

    let mut out = String::with_capacity(template.len());
    let mut i = 0usize;

    while i < template.len() {
        let Some(ch) = template[i..].chars().next() else {
            break;
        };

        if let Some(consumed) = copy_html_comment(template, i, &mut out) {
            i += consumed;
            continue;
        }

        if ch == '<'
            && let Some(invocation) = parse_component_invocation(&template[i..])
        {
            let import_target = owner_module.imports.get(&invocation.name).ok_or_else(|| {
                let (line, col) = line_col_at(template, i);
                let mut msg = format!(
                    "missing explicit import for component `<{}>` at template line {line}, column {col}.",
                    invocation.name
                );
                let suggestions = suggested_import_paths(&invocation.name, modules);
                if suggestions.is_empty() {
                    msg.push_str(&format!(
                        " Add `import {} from \"ui/...\";` in frontmatter.",
                        invocation.name
                    ));
                } else if suggestions.len() == 1 {
                    msg.push_str(&format!(
                        " Add `import {} from \"{}\";` in frontmatter.",
                        invocation.name, suggestions[0]
                    ));
                } else {
                    msg.push_str(" Add one of the following imports in frontmatter:");
                    for suggestion in suggestions {
                        msg.push_str(&format!(
                            "\n    import {} from \"{}\";",
                            invocation.name, suggestion
                        ));
                    }
                }
                template_compile_error(owner_path, msg)
            })?;

            let imported_module = modules.get(import_target).ok_or_else(|| {
                template_compile_error(
                    owner_path,
                    format!(
                        "import target `{}` was not part of discovered templates",
                        src_relative_display_path(import_target)
                    ),
                )
            })?;

            if let Some(cycle_start) = stack.iter().position(|path| path == import_target) {
                let mut cycle_chain = stack[cycle_start..]
                    .iter()
                    .map(|p| src_relative_display_path(p))
                    .collect::<Vec<_>>();
                cycle_chain.push(src_relative_display_path(import_target));
                return Err(template_compile_error(
                    owner_path,
                    format!(
                        "component import cycle detected: {}",
                        cycle_chain.join(" -> ")
                    ),
                ));
            }

            let inner_expanded = invocation
                .inner
                .as_deref()
                .map(|inner| expand_known_components(inner, owner_path, modules, stack, depth + 1))
                .transpose()?
                .unwrap_or_default();

            let slot_assignments = collect_slot_assignments(&inner_expanded);
            let component_with_slot =
                apply_slots(&imported_module.template_source, &slot_assignments);

            stack.push(import_target.clone());
            let component_body = expand_known_components(
                &component_with_slot,
                import_target,
                modules,
                stack,
                depth + 1,
            )?;
            stack.pop();

            out.push_str(&render_askama_let_bindings(&invocation.attrs));
            out.push_str(&component_body);
            i += invocation.consumed;
            continue;
        }

        out.push(ch);
        i += ch.len_utf8();
    }

    Ok(out)
}

fn copy_html_comment(input: &str, start: usize, output: &mut String) -> Option<usize> {
    if !input[start..].starts_with("<!--") {
        return None;
    }

    let consumed = match input[start + 4..].find("-->") {
        Some(end) => 4 + end + 3,
        None => input.len() - start,
    };
    output.push_str(&input[start..start + consumed]);
    Some(consumed)
}

fn render_askama_let_bindings(attrs: &[(String, String)]) -> String {
    let mut out = String::new();
    for (name, expr) in attrs {
        let expr = expr.trim();
        if expr == name {
            continue;
        }
        out.push_str("{% let ");
        out.push_str(name);
        out.push_str(" = ");
        out.push('(');
        out.push_str(expr);
        out.push_str(").clone()");
        out.push_str(" %}");
    }
    out
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SlotAssignments {
    default: Vec<SlotFragment>,
    named: HashMap<String, Vec<SlotFragment>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SlotFragment {
    content: String,
    let_bindings: Vec<String>,
}

fn collect_slot_assignments(inner: &str) -> SlotAssignments {
    let mut assignments = SlotAssignments::default();
    let mut idx = 0usize;

    while idx < inner.len() {
        if let Some(node) = parse_html_node_at(inner, idx) {
            // <pilcrow:head> is a built-in named slot: its inner content targets "pilcrow_head"
            let effective_slot = if node.name.eq_ignore_ascii_case("pilcrow:head") {
                Some("pilcrow_head".to_string())
            } else {
                extract_slot_name(&node.attrs)
            };

            if let Some(slot_name) = effective_slot {
                let let_bindings = extract_slot_let_bindings(&node.attrs);
                let content = render_slot_fragment_node(inner, &node);
                let fragment = SlotFragment {
                    content,
                    let_bindings,
                };

                if slot_name == "default" {
                    assignments.default.push(fragment);
                } else {
                    assignments
                        .named
                        .entry(slot_name)
                        .or_default()
                        .push(fragment);
                }
            } else {
                assignments.default.push(SlotFragment {
                    content: inner[idx..idx + node.consumed].to_string(),
                    let_bindings: Vec::new(),
                });
            }
            idx += node.consumed;
            continue;
        }

        // Text chunk until next potential tag. If the current byte is `<` but
        // not a parseable HTML node, keep it as text (e.g. HTML comments).
        let search_from = if inner[idx..].starts_with('<') {
            idx + 1
        } else {
            idx
        };
        let next_tag = inner[search_from..]
            .find('<')
            .map(|off| search_from + off)
            .unwrap_or(inner.len());
        if next_tag > idx {
            assignments.default.push(SlotFragment {
                content: inner[idx..next_tag].to_string(),
                let_bindings: Vec::new(),
            });
        }
        idx = next_tag;
    }

    assignments
}

fn render_slot_fragment_node(source: &str, node: &HtmlNode) -> String {
    if node.name.eq_ignore_ascii_case("Fragment") || node.name.eq_ignore_ascii_case("pilcrow:head")
    {
        return node
            .inner
            .map(|(start, end)| source[start..end].to_string())
            .unwrap_or_default();
    }

    let attrs = node
        .attrs
        .iter()
        .filter(|a| a.name != "slot" && !a.name.starts_with("let:"))
        .map(render_html_attr)
        .collect::<String>();

    if node.self_closing {
        return format!("<{}{} />", node.name, attrs);
    }

    let inner = node
        .inner
        .map(|(start, end)| &source[start..end])
        .unwrap_or_default();
    format!("<{}{}>{}</{}>", node.name, attrs, inner, node.name)
}

fn apply_slots(component_template: &str, assignments: &SlotAssignments) -> String {
    let mut out = String::new();
    let mut idx = 0usize;

    while idx < component_template.len() {
        if let Some(slot_tag) = parse_slot_tag_at(component_template, idx) {
            let replacement = render_slot_replacement(assignments, &slot_tag);
            out.push_str(&replacement);
            idx += slot_tag.consumed;
            continue;
        }

        let Some(ch) = component_template[idx..].chars().next() else {
            break;
        };
        out.push(ch);
        idx += ch.len_utf8();
    }

    out
}

fn render_slot_replacement(assignments: &SlotAssignments, slot: &SlotTag) -> String {
    let fragments = if slot.name == "default" {
        assignments.default.as_slice()
    } else {
        assignments
            .named
            .get(&slot.name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    };

    if fragments.is_empty() {
        return slot.fallback.clone().unwrap_or_default();
    }

    let mut out = String::new();
    for fragment in fragments {
        for binding in &fragment.let_bindings {
            if let Some(expr) = slot.props.get(binding) {
                out.push_str("{% let ");
                out.push_str(binding);
                out.push_str(" = ");
                out.push('(');
                out.push_str(expr);
                out.push_str(").clone()");
                out.push_str(" %}");
            }
        }
        out.push_str(&fragment.content);
    }
    out
}

fn extract_slot_name(attrs: &[HtmlAttr]) -> Option<String> {
    attrs
        .iter()
        .find(|a| a.name == "slot")
        .and_then(|a| a.value.clone())
        .map(|v| strip_wrapping_quotes(&v).to_string())
}

fn extract_slot_let_bindings(attrs: &[HtmlAttr]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|a| a.name.strip_prefix("let:").map(|s| s.to_string()))
        .collect()
}

fn render_html_attr(attr: &HtmlAttr) -> String {
    match (&attr.value, &attr.kind) {
        (None, _) => format!(" {}", attr.name),
        (Some(value), HtmlAttrKind::DoubleQuoted) => format!(" {}=\"{}\"", attr.name, value),
        (Some(value), HtmlAttrKind::SingleQuoted) => format!(" {}='{}'", attr.name, value),
        (Some(value), HtmlAttrKind::Braced) => format!(" {}={{{}}}", attr.name, value),
        (Some(value), HtmlAttrKind::Bare) => format!(" {}={}", attr.name, value),
    }
}

fn strip_wrapping_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn is_pascal_case_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_uppercase() && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlNode {
    name: String,
    attrs: Vec<HtmlAttr>,
    self_closing: bool,
    consumed: usize,
    inner: Option<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlAttr {
    name: String,
    value: Option<String>,
    kind: HtmlAttrKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HtmlAttrKind {
    DoubleQuoted,
    SingleQuoted,
    Braced,
    Bare,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SlotTag {
    name: String,
    props: HashMap<String, String>,
    fallback: Option<String>,
    consumed: usize,
}

fn parse_html_node_at(input: &str, start: usize) -> Option<HtmlNode> {
    let open = parse_html_open_tag_at(input, start)?;
    if open.self_closing {
        return Some(HtmlNode {
            name: open.name,
            attrs: open.attrs,
            self_closing: true,
            consumed: open.consumed,
            inner: None,
        });
    }

    let open_end = start + open.consumed;
    let (close_start, close_end) = find_matching_html_close(input, open_end, &open.name)?;
    Some(HtmlNode {
        name: open.name,
        attrs: open.attrs,
        self_closing: false,
        consumed: close_end - start,
        inner: Some((open_end, close_start)),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlOpenTag {
    name: String,
    attrs: Vec<HtmlAttr>,
    self_closing: bool,
    consumed: usize,
}

fn parse_html_open_tag_at(input: &str, start: usize) -> Option<HtmlOpenTag> {
    if !input[start..].starts_with('<') {
        return None;
    }

    let mut idx = start + 1;
    let first = input[idx..].chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if input[idx..].starts_with('/')
        || input[idx..].starts_with('!')
        || input[idx..].starts_with('?')
    {
        return None;
    }

    let mut name_end = idx + first.len_utf8();
    while let Some(c) = input[name_end..].chars().next() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':') {
            name_end += c.len_utf8();
        } else {
            break;
        }
    }

    let name = input[idx..name_end].to_string();
    idx = name_end;

    let attrs_start = idx;
    let mut brace_depth = 0usize;
    let mut quote: Option<char> = None;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => {
                quote = Some(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                idx += c_len;
            }
            '}' => {
                if brace_depth == 0 {
                    return None;
                }
                brace_depth -= 1;
                idx += c_len;
            }
            '>' if brace_depth == 0 => {
                let attrs_src = &input[attrs_start..idx];
                let attrs = parse_html_attrs(attrs_src)?;
                let before = attrs_src.trim_end();
                let self_closing = before.ends_with('/');
                return Some(HtmlOpenTag {
                    name,
                    attrs,
                    self_closing,
                    consumed: idx + c_len - start,
                });
            }
            _ => idx += c_len,
        }
    }

    None
}

fn find_matching_html_close(input: &str, from: usize, name: &str) -> Option<(usize, usize)> {
    let mut idx = from;
    let mut depth = 1usize;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        if c != '<' {
            idx += c.len_utf8();
            continue;
        }

        if let Some(consumed) = parse_named_close_tag(input, idx, name) {
            depth -= 1;
            if depth == 0 {
                return Some((idx, idx + consumed));
            }
            idx += consumed;
            continue;
        }

        if let Some((consumed, self_closing)) = parse_named_open_tag(input, idx, name) {
            if !self_closing {
                depth += 1;
            }
            idx += consumed;
            continue;
        }

        idx += c.len_utf8();
    }

    None
}

fn parse_html_attrs(src: &str) -> Option<Vec<HtmlAttr>> {
    let mut attrs = Vec::new();
    let mut idx = 0usize;

    while idx < src.len() {
        idx = skip_ws(src, idx);
        if idx >= src.len() {
            break;
        }
        if src[idx..].starts_with('/') {
            // Self-closing marker in tags like `<slot />`
            break;
        }

        let (name, next_idx) = parse_html_attr_name(src, idx)?;
        idx = skip_ws(src, next_idx);

        if idx >= src.len() || !src[idx..].starts_with('=') {
            attrs.push(HtmlAttr {
                name,
                value: None,
                kind: HtmlAttrKind::Bare,
            });
            continue;
        }

        idx += 1;
        idx = skip_ws(src, idx);
        let (value, kind, consumed_to) = parse_html_attr_value(src, idx)?;
        attrs.push(HtmlAttr {
            name,
            value: Some(value),
            kind,
        });
        idx = consumed_to;
    }

    Some(attrs)
}

fn parse_html_attr_name(src: &str, start: usize) -> Option<(String, usize)> {
    let first = src[start..].chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '_' || first == ':') {
        return None;
    }

    let mut idx = start + first.len_utf8();
    while let Some(c) = src[idx..].chars().next() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '-' | '.') {
            idx += c.len_utf8();
        } else {
            break;
        }
    }

    Some((src[start..idx].to_string(), idx))
}

fn parse_html_attr_value(src: &str, start: usize) -> Option<(String, HtmlAttrKind, usize)> {
    let first = src[start..].chars().next()?;
    match first {
        '"' => {
            let mut idx = start + 1;
            while idx < src.len() {
                let c = src[idx..].chars().next()?;
                if c == '"' {
                    return Some((
                        src[start + 1..idx].to_string(),
                        HtmlAttrKind::DoubleQuoted,
                        idx + 1,
                    ));
                }
                idx += c.len_utf8();
            }
            None
        }
        '\'' => {
            let mut idx = start + 1;
            while idx < src.len() {
                let c = src[idx..].chars().next()?;
                if c == '\'' {
                    return Some((
                        src[start + 1..idx].to_string(),
                        HtmlAttrKind::SingleQuoted,
                        idx + 1,
                    ));
                }
                idx += c.len_utf8();
            }
            None
        }
        '{' => {
            let (expr, end) = parse_braced_expr(src, start)?;
            Some((expr, HtmlAttrKind::Braced, end))
        }
        _ => {
            let mut idx = start;
            while let Some(c) = src[idx..].chars().next() {
                if c.is_whitespace() || c == '>' {
                    break;
                }
                idx += c.len_utf8();
            }
            Some((src[start..idx].to_string(), HtmlAttrKind::Bare, idx))
        }
    }
}

fn parse_slot_tag_at(input: &str, start: usize) -> Option<SlotTag> {
    let open = parse_html_open_tag_at(input, start)?;
    if !open.name.eq_ignore_ascii_case("slot") {
        return None;
    }

    let mut props = HashMap::new();
    let mut slot_name = "default".to_string();
    for attr in open.attrs {
        if attr.name == "name" {
            if let Some(value) = attr.value {
                slot_name = strip_wrapping_quotes(&value).to_string();
            }
            continue;
        }

        let Some(value) = attr.value else {
            continue;
        };
        let expr = match attr.kind {
            HtmlAttrKind::Braced => value,
            HtmlAttrKind::DoubleQuoted | HtmlAttrKind::SingleQuoted | HtmlAttrKind::Bare => {
                askama_expr_or_string_literal(&value)
            }
        };
        props.insert(attr.name, expr);
    }

    if open.self_closing {
        return Some(SlotTag {
            name: slot_name,
            props,
            fallback: None,
            consumed: open.consumed,
        });
    }

    let open_end = start + open.consumed;
    let (close_start, close_end) = find_matching_html_close(input, open_end, "slot")?;
    Some(SlotTag {
        name: slot_name,
        props,
        fallback: Some(input[open_end..close_start].to_string()),
        consumed: close_end - start,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedComponentInvocation {
    name: String,
    attrs: Vec<(String, String)>,
    inner: Option<String>,
    consumed: usize,
}

fn parse_component_invocation(input: &str) -> Option<ParsedComponentInvocation> {
    let mut idx = 0usize;
    idx += consume_char(input, idx, '<')?;

    let first = input[idx..].chars().next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }

    let mut name_end = idx + first.len_utf8();
    while let Some(c) = input[name_end..].chars().next() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name_end += c.len_utf8();
        } else {
            break;
        }
    }

    let name = input[idx..name_end].to_string();

    // Fragment is a built-in slot wrapper, not a user component.
    // Let it pass through as plain HTML so the slot system can handle it.
    if name == "Fragment" {
        return None;
    }

    idx = name_end;

    let attrs_start = idx;
    let mut brace_depth = 0usize;
    let mut quote: Option<char> = None;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => {
                quote = Some(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                idx += c_len;
            }
            '}' => {
                if brace_depth == 0 {
                    return None;
                }
                brace_depth -= 1;
                idx += c_len;
            }
            '/' if brace_depth == 0 && input[idx..].starts_with("/>") => {
                let attrs = parse_attributes(&input[attrs_start..idx])?;
                return Some(ParsedComponentInvocation {
                    name,
                    attrs,
                    inner: None,
                    consumed: idx + 2,
                });
            }
            '>' => {
                let attrs = parse_attributes(&input[attrs_start..idx])?;
                let open_end = idx + c_len;
                let (inner_len, close_len) =
                    find_matching_component_close(&input[open_end..], &name)?;
                let inner = input[open_end..open_end + inner_len].to_string();
                return Some(ParsedComponentInvocation {
                    name,
                    attrs,
                    inner: Some(inner),
                    consumed: open_end + inner_len + close_len,
                });
            }
            _ => idx += c_len,
        }
    }

    None
}

fn find_matching_component_close(input: &str, name: &str) -> Option<(usize, usize)> {
    let mut idx = 0usize;
    let mut depth = 1usize;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        if c != '<' {
            idx += c.len_utf8();
            continue;
        }

        if let Some(consumed) = parse_named_close_tag(input, idx, name) {
            depth -= 1;
            if depth == 0 {
                return Some((idx, consumed));
            }
            idx += consumed;
            continue;
        }

        if let Some((consumed, self_closing)) = parse_named_open_tag(input, idx, name) {
            if !self_closing {
                depth += 1;
            }
            idx += consumed;
            continue;
        }

        idx += c.len_utf8();
    }

    None
}

fn parse_named_open_tag(input: &str, start: usize, name: &str) -> Option<(usize, bool)> {
    if !input[start..].starts_with('<') {
        return None;
    }

    let mut idx = start + 1;
    if input[idx..].starts_with('/') {
        return None;
    }
    if !input[idx..].starts_with(name) {
        return None;
    }
    idx += name.len();

    let boundary = input[idx..].chars().next()?;
    if !(boundary.is_whitespace() || boundary == '/' || boundary == '>') {
        return None;
    }

    let attrs_start = idx;
    let mut brace_depth = 0usize;
    let mut quote: Option<char> = None;

    while idx < input.len() {
        let c = input[idx..].chars().next()?;
        let c_len = c.len_utf8();

        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }

        match c {
            '"' | '\'' => {
                quote = Some(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                idx += c_len;
            }
            '}' => {
                if brace_depth == 0 {
                    return None;
                }
                brace_depth -= 1;
                idx += c_len;
            }
            '>' if brace_depth == 0 => {
                let before = input[attrs_start..idx].trim_end();
                let self_closing = before.ends_with('/');
                return Some((idx + c_len - start, self_closing));
            }
            _ => idx += c_len,
        }
    }

    None
}

fn parse_named_close_tag(input: &str, start: usize, name: &str) -> Option<usize> {
    if !input[start..].starts_with("</") {
        return None;
    }

    let mut idx = start + 2;
    if !input[idx..].starts_with(name) {
        return None;
    }
    idx += name.len();

    let boundary = input[idx..].chars().next()?;
    if !(boundary.is_whitespace() || boundary == '>') {
        return None;
    }

    idx = skip_ws(input, idx);
    if !input[idx..].starts_with('>') {
        return None;
    }

    Some(idx + 1 - start)
}

fn parse_attributes(src: &str) -> Option<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut idx = 0usize;

    while idx < src.len() {
        idx = skip_ws(src, idx);
        if idx >= src.len() {
            break;
        }

        let (name, next_idx) = parse_attr_name(src, idx)?;
        idx = skip_ws(src, next_idx);

        if idx >= src.len() || !src[idx..].starts_with('=') {
            out.push((name, "true".to_string()));
            continue;
        }

        idx += 1;
        idx = skip_ws(src, idx);
        let (expr, consumed_to) = parse_attr_value(src, idx)?;
        out.push((name, expr));
        idx = consumed_to;
    }

    Some(out)
}

fn parse_attr_name(src: &str, start: usize) -> Option<(String, usize)> {
    let first = src[start..].chars().next()?;
    if !is_rust_ident_start(first) {
        return None;
    }

    let mut idx = start + first.len_utf8();
    while let Some(c) = src[idx..].chars().next() {
        if is_rust_ident_continue(c) {
            idx += c.len_utf8();
        } else {
            break;
        }
    }

    Some((src[start..idx].to_string(), idx))
}

fn parse_attr_value(src: &str, start: usize) -> Option<(String, usize)> {
    let first = src[start..].chars().next()?;
    match first {
        '{' => parse_braced_expr(src, start),
        '"' | '\'' => parse_quoted_expr(src, start, first),
        _ => {
            let mut idx = start;
            while let Some(c) = src[idx..].chars().next() {
                if c.is_whitespace() {
                    break;
                }
                idx += c.len_utf8();
            }
            Some((src[start..idx].to_string(), idx))
        }
    }
}

fn parse_braced_expr(src: &str, start: usize) -> Option<(String, usize)> {
    let mut idx = start + 1;
    let mut depth = 1usize;

    while idx < src.len() {
        let c = src[idx..].chars().next()?;
        let c_len = c.len_utf8();
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let expr = src[start + 1..idx].trim().to_string();
                    return Some((expr, idx + c_len));
                }
            }
            _ => {}
        }
        idx += c_len;
    }

    None
}

fn parse_quoted_expr(src: &str, start: usize, quote: char) -> Option<(String, usize)> {
    let mut idx = start + quote.len_utf8();
    while idx < src.len() {
        let c = src[idx..].chars().next()?;
        let c_len = c.len_utf8();
        if c == quote {
            let raw = &src[start + quote.len_utf8()..idx];
            let expr = askama_expr_or_string_literal(raw);
            return Some((expr, idx + c_len));
        }
        idx += c_len;
    }
    None
}

fn askama_expr_or_string_literal(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed
        .strip_prefix("{{")
        .and_then(|v| v.strip_suffix("}}"))
    {
        return inner.trim().to_string();
    }
    format!("{raw:?}")
}

fn skip_ws(src: &str, mut idx: usize) -> usize {
    while idx < src.len() {
        let Some(c) = src[idx..].chars().next() else {
            break;
        };
        if c.is_whitespace() {
            idx += c.len_utf8();
        } else {
            break;
        }
    }
    idx
}

fn consume_char(src: &str, idx: usize, expected: char) -> Option<usize> {
    let c = src[idx..].chars().next()?;
    if c == expected {
        Some(c.len_utf8())
    } else {
        None
    }
}

fn is_rust_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_rust_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn compile_pipeline_writes_transpiled_templates_and_routes() {
        let root = mk_temp_root("compile_pipeline_ok");
        let src = root.join("src");
        let out = root.join("out");

        // Root auto-layout wraps all pages automatically.
        write_file(
            &src.join("pages/_layout.html"),
            r#"---
pub struct Props {}
---
<html><body><slot /></body></html>"#,
        );
        write_file(
            &src.join("pages/index.html"),
            r#"---
import Card from "ui/Card.html";

pub struct Props {
    pub title: String,
}
---
<Card title={title} />"#,
        );
        write_file(
            &src.join("ui/Card.html"),
            r#"---
pub struct Props {
    pub title: String,
}
---
<article>{{ title }}</article>"#,
        );

        let result = compile_to_out_dir(&src, &out).expect("pipeline should compile");

        // auto-layout + page + ui card = 3 modules
        assert_eq!(result.preprocessed_files.len(), 3);
        assert!(result.generated_routes_file.exists());
        assert!(result.generated_templates_file.exists());
        assert!(result.generated_routes.iter().any(|r| r.pattern == "/"));
        assert!(
            result
                .generated_templates
                .iter()
                .any(|t| t.render_symbol == "render_page_index")
        );

        let page_template = out.join("pilcrow_templates/pages/index.html");
        let page_rendered = fs::read_to_string(page_template).expect("read transpiled page");
        assert!(page_rendered.contains("<html><body>"));
        assert!(page_rendered.contains("<article>{{ title }}</article>"));
        assert!(!page_rendered.contains("<Layout"));
        assert!(!page_rendered.contains("{{ Layout {"));
        assert!(!page_rendered.contains("{{ Card {"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_expands_named_slots() {
        let root = mk_temp_root("compile_named_slots");
        let src = root.join("src");
        let out = root.join("out");

        // Auto-layout with named slots; page content is passed through slots.
        write_file(
            &src.join("pages/_layout.html"),
            r#"---
pub struct Props {}
---
<header><slot name="header" /></header>
<main><slot /></main>"#,
        );
        write_file(
            &src.join("pages/index.html"),
            r#"---
pub struct Props {}
---
<h1 slot="header">Top</h1>
<p>Body</p>"#,
        );

        let _result = compile_to_out_dir(&src, &out).expect("pipeline should compile");
        let page_template = out.join("pilcrow_templates/pages/index.html");
        let page_rendered = fs::read_to_string(page_template).expect("read transpiled page");

        assert!(page_rendered.contains("<header><h1>Top</h1></header>"));
        assert!(page_rendered.contains("<main>"));
        assert!(page_rendered.contains("<p>Body</p>"));
        assert!(!page_rendered.contains("slot=\"header\""));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_preserves_html_comments_through_layout_slots() {
        let root = mk_temp_root("compile_comments_through_slots");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/_layout.html"),
            r#"---
pub struct Props {}
---
<html><body><slot /></body></html>"#,
        );
        write_file(
            &src.join("pages/index.html"),
            r#"---
pub struct Props {}
---
<p>Before</p>
<!--
<island src="./hidden" />
<Card title={hidden} />
-->
<p>After</p>"#,
        );

        compile_to_out_dir(&src, &out).expect("pipeline should compile");
        let page_template = out.join("pilcrow_templates/pages/index.html");
        let page_rendered = fs::read_to_string(page_template).expect("read transpiled page");

        assert!(page_rendered.contains("<!--\n<island src=\"./hidden\" />"));
        assert!(page_rendered.contains("<Card title={hidden} />\n-->"));
        assert!(page_rendered.contains("<p>Before</p>"));
        assert!(page_rendered.contains("<p>After</p>"));
        assert!(!page_rendered.contains("data-pilcrow-island"));
        assert!(!page_rendered.contains("<body>!--\n<island"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_expands_slot_props_via_let_bindings() {
        let root = mk_temp_root("compile_slot_props");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/index.html"),
            r#"---
import List from "ui/List.html";

pub struct Props {
    pub title: String,
}
---
<List title={title}>
    <li slot="item" let:item>{{ item }}</li>
</List>"#,
        );
        write_file(
            &src.join("ui/List.html"),
            r#"---
pub struct Props {
    pub title: String,
}
---
<ul><slot name="item" item={title} /></ul>"#,
        );

        let _result = compile_to_out_dir(&src, &out).expect("pipeline should compile");
        let page_template = out.join("pilcrow_templates/pages/index.html");
        let page_rendered = fs::read_to_string(page_template).expect("read transpiled page");

        assert!(page_rendered.contains("{% let item = (title).clone() %}<li>{{ item }}</li>"));
        assert!(page_rendered.contains("<ul>"));
        assert!(!page_rendered.contains("let:item"));
        assert!(!page_rendered.contains("slot=\"item\""));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_fails_on_missing_component_import() {
        let root = mk_temp_root("compile_missing_component_import");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/index.html"),
            r#"---
pub struct Props {}
---
<UnknownWidget title={title} />"#,
        );

        let err = compile_to_out_dir(&src, &out).expect_err("pipeline should fail");
        let msg = err.to_string();
        assert!(msg.contains("file: pages/index.html"));
        assert!(msg.contains("line 1, column 1"));
        assert!(msg.contains("missing explicit import"));
        assert!(msg.contains("<UnknownWidget>"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_fails_on_invalid_import_path() {
        let root = mk_temp_root("compile_invalid_import_path");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/index.html"),
            r#"---
import Card from "pages/Card.html";
pub struct Props {}
---
<Card />"#,
        );

        let err = compile_to_out_dir(&src, &out).expect_err("pipeline should fail");
        let msg = err.to_string();
        assert!(msg.contains("file: pages/index.html"));
        assert!(msg.contains("unknown import alias `pages`"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_fails_on_duplicate_import_alias() {
        let root = mk_temp_root("compile_duplicate_import_alias");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/index.html"),
            r#"---
import Card from "ui/Card.html";
import Card from "ui/OtherCard.html";
pub struct Props {}
---
<Card />"#,
        );
        write_file(
            &src.join("ui/Card.html"),
            r#"---
pub struct Props {}
---
<div>Card</div>"#,
        );
        write_file(
            &src.join("ui/OtherCard.html"),
            r#"---
pub struct Props {}
---
<div>Other</div>"#,
        );

        let err = compile_to_out_dir(&src, &out).expect_err("pipeline should fail");
        let msg = err.to_string();
        assert!(msg.contains("file: pages/index.html"));
        assert!(msg.contains("duplicate import alias `Card`"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_fails_on_component_import_cycle() {
        let root = mk_temp_root("compile_import_cycle");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/index.html"),
            r#"---
import A from "ui/A.html";
pub struct Props {}
---
<A />"#,
        );
        write_file(
            &src.join("ui/A.html"),
            r#"---
import B from "ui/B.html";
pub struct Props {}
---
<B />"#,
        );
        write_file(
            &src.join("ui/B.html"),
            r#"---
import A from "ui/A.html";
pub struct Props {}
---
<A />"#,
        );

        let err = compile_to_out_dir(&src, &out).expect_err("pipeline should fail");
        let msg = err.to_string();
        assert!(msg.contains("file:"));
        assert!(msg.contains("component import cycle detected"));
        assert!(msg.contains("ui/A.html"));
        assert!(msg.contains("ui/B.html"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_requires_nested_component_file_imports() {
        let root = mk_temp_root("compile_nested_component_imports");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/index.html"),
            r#"---
import ParentCard from "ui/ParentCard.html";
pub struct Props {}
---
<ParentCard />"#,
        );
        write_file(
            &src.join("ui/ParentCard.html"),
            r#"---
pub struct Props {}
---
<StatusBadge text="nested" />"#,
        );
        write_file(
            &src.join("ui/StatusBadge.html"),
            r#"---
pub struct Props {
    pub text: String,
}
---
<span>{{ text }}</span>"#,
        );

        let err = compile_to_out_dir(&src, &out).expect_err("pipeline should fail");
        let msg = err.to_string();
        assert!(msg.contains("file: ui/ParentCard.html"));
        assert!(msg.contains("line 1, column 1"));
        assert!(msg.contains("missing explicit import"));
        assert!(msg.contains("<StatusBadge>"));
        assert!(msg.contains("ui/ParentCard.html"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_accepts_fenceless_html_as_static_page() {
        let root = mk_temp_root("compile_pipeline_fenceless");
        let src = root.join("src");
        let out = root.join("out");

        // No `---` frontmatter fences → treated as a pure static page.
        // Routekit synthesizes an empty `pub struct Props;`.
        write_file(&src.join("pages/index.html"), "<h1>Missing fences</h1>");

        let output = compile_to_out_dir(&src, &out).expect("fenceless page should compile");
        let entry = output
            .preprocessed_files
            .iter()
            .find(|f| f.module_name == "page_index")
            .expect("page_index should be present");
        assert_eq!(entry.rust_frontmatter, "");
        assert_eq!(entry.transpiled_template, "<h1>Missing fences</h1>");

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_code_behind_rs_file_provides_rust_frontmatter() {
        let root = mk_temp_root("compile_codebehind_ok");
        let src = root.join("src");
        let out = root.join("out");

        // .html has no frontmatter (auto-layout wraps it); .rs has Props + load()
        write_file(&src.join("pages/index.html"), "<h1>{{ title }}</h1>");
        write_file(
            &src.join("pages/index.rs"),
            r#"pub struct Props {
    pub title: String,
}
pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props { title: "Home".to_string() })
}"#,
        );

        let result = compile_to_out_dir(&src, &out).expect("code-behind pipeline should compile");
        let page = result
            .preprocessed_files
            .iter()
            .find(|f| f.module_name == "page_index")
            .expect("page_index should exist");
        assert!(page.rust_frontmatter.contains("pub struct Props"));
        assert!(page.rust_frontmatter.contains("pub async fn load"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_code_behind_rejects_rust_in_html_frontmatter() {
        let root = mk_temp_root("compile_codebehind_err");
        let src = root.join("src");
        let out = root.join("out");

        // Both .html frontmatter and .rs file have Rust — should error
        write_file(
            &src.join("pages/index.html"),
            r#"---
pub struct Props {}
---
<h1>Hello</h1>"#,
        );
        write_file(&src.join("pages/index.rs"), r#"pub struct Props {}"#);

        let err = compile_to_out_dir(&src, &out).expect_err("mixed frontmatter should fail");
        let msg = err.to_string();
        assert!(msg.contains("frontmatter may only contain"));
        assert!(msg.contains("code-behind"));

        cleanup(&root);
    }

    #[test]
    fn watched_dirs_are_pages_ui_api() {
        let src = PathBuf::from("/tmp/project/src");
        let dirs = watched_source_directories(&src);
        assert_eq!(dirs[0], PathBuf::from("/tmp/project/src/pages"));
        assert_eq!(dirs[1], PathBuf::from("/tmp/project/src/ui"));
        assert_eq!(dirs[2], PathBuf::from("/tmp/project/src/api"));
        assert_eq!(dirs[3], PathBuf::from("/tmp/project/src/params"));
    }

    #[test]
    fn compile_pipeline_layout_group_strips_group_from_url_and_module() {
        let root = mk_temp_root("layout_group");
        let src = root.join("src");
        let out = root.join("out");

        // (public) group: layout group directory invisible in URLs
        write_file(
            &src.join("pages/(public)/_layout.html"),
            r#"---
pub struct Props {}
---
<div class="public"><slot /></div>"#,
        );
        write_file(&src.join("pages/(public)/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/(public)/about.html"), "<h1>About</h1>");

        // (app) group: different layout
        write_file(
            &src.join("pages/(app)/_layout.html"),
            r#"---
pub struct Props {}
---
<div class="app"><slot /></div>"#,
        );
        write_file(
            &src.join("pages/(app)/dashboard.html"),
            "<h1>Dashboard</h1>",
        );

        let result = compile_to_out_dir(&src, &out).expect("layout groups should compile");

        // URLs should not contain the group name
        let patterns: Vec<_> = result
            .generated_routes
            .iter()
            .map(|r| r.pattern.as_str())
            .collect();
        assert!(patterns.contains(&"/"), "index → /");
        assert!(patterns.contains(&"/about"), "about → /about");
        assert!(patterns.contains(&"/dashboard"), "dashboard → /dashboard");
        assert!(
            patterns.iter().all(|p| !p.contains("public")),
            "no group in URL"
        );
        assert!(
            patterns.iter().all(|p| !p.contains("app")),
            "no group in URL"
        );

        // Module symbols should also strip the group
        let symbols: Vec<_> = result
            .generated_routes
            .iter()
            .map(|r| r.symbol.as_str())
            .collect();
        assert!(symbols.contains(&"page_index"));
        assert!(symbols.contains(&"page_about"));
        assert!(symbols.contains(&"page_dashboard"));

        // Each page should be wrapped by its group's _layout.html
        let index_tpl = out.join("pilcrow_templates/pages/(public)/index.html");
        let index_rendered = fs::read_to_string(index_tpl).expect("read index");
        assert!(
            index_rendered.contains(r#"class="public""#),
            "public layout applied to index"
        );
        assert!(!index_rendered.contains(r#"class="app""#));

        let dashboard_tpl = out.join("pilcrow_templates/pages/(app)/dashboard.html");
        let dashboard_rendered = fs::read_to_string(dashboard_tpl).expect("read dashboard");
        assert!(
            dashboard_rendered.contains(r#"class="app""#),
            "app layout applied to dashboard"
        );
        assert!(!dashboard_rendered.contains(r#"class="public""#));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_rejects_load_without_req() {
        let root = mk_temp_root("load_missing_req");
        let src = root.join("src");
        let out = root.join("out");

        write_file(&src.join("pages/index.html"), "<h1>{{ title }}</h1>");
        write_file(
            &src.join("pages/index.rs"),
            r#"pub struct Props { pub title: String }
pub async fn load() -> AppResult<Props> {
    Ok(Props { title: "oops".to_string() })
}"#,
        );

        let err = compile_to_out_dir(&src, &out).expect_err("missing Req should fail");
        let msg = err.to_string();
        assert!(msg.contains("must take `req: Req`"), "got: {msg}");
        assert!(msg.contains("pages/index"), "got: {msg}");

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_rejects_non_async_load() {
        let root = mk_temp_root("load_not_async");
        let src = root.join("src");
        let out = root.join("out");

        write_file(&src.join("pages/index.html"), "<h1>hi</h1>");
        write_file(
            &src.join("pages/index.rs"),
            r#"pub struct Props {}
pub fn load(_req: Req) -> AppResult<Props> { Ok(Props {}) }"#,
        );

        let err = compile_to_out_dir(&src, &out).expect_err("non-async load should fail");
        let msg = err.to_string();
        assert!(msg.contains("must be declared `async`"), "got: {msg}");

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_rejects_load_without_result_return() {
        let root = mk_temp_root("load_no_result");
        let src = root.join("src");
        let out = root.join("out");

        write_file(&src.join("pages/index.html"), "<h1>hi</h1>");
        write_file(
            &src.join("pages/index.rs"),
            r#"pub struct Props {}
pub async fn load(_req: Req) -> Props { Props {} }"#,
        );

        let err = compile_to_out_dir(&src, &out).expect_err("non-Result load should fail");
        let msg = err.to_string();
        assert!(msg.contains("must return `AppResult<Props>`"), "got: {msg}");

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_processes_markdown_page() {
        let root = mk_temp_root("markdown_page");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/posts/hello.md"),
            "# Hello World\n\nThis is a **markdown** page.\n",
        );

        let result = compile_to_out_dir(&src, &out).expect("pipeline should compile markdown");

        assert!(
            result
                .generated_routes
                .iter()
                .any(|r| r.pattern == "/posts/hello"),
            "expected route /posts/hello, got: {:?}",
            result
                .generated_routes
                .iter()
                .map(|r| &r.pattern)
                .collect::<Vec<_>>()
        );

        let tpl_path = out.join("pilcrow_templates/pages/posts/hello.html");
        assert!(
            tpl_path.exists(),
            "transpiled template should exist as .html"
        );
        let rendered = fs::read_to_string(&tpl_path).expect("read transpiled markdown template");
        assert!(
            rendered.contains("<h1>Hello World</h1>"),
            "H1 should be rendered: {rendered}"
        );
        assert!(
            rendered.contains("<strong>markdown</strong>"),
            "bold should be rendered: {rendered}"
        );

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_markdown_with_codebehind() {
        let root = mk_temp_root("markdown_codebehind");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/blog.md"),
            "# My Blog\n\nWelcome to {{ title }}.\n",
        );
        write_file(
            &src.join("pages/blog.rs"),
            "pub struct Props {\n    pub title: String,\n}\n\npub async fn load(_req: Req) -> AppResult<Props> {\n    Ok(Props { title: \"Pilcrow Blog\".to_string() })\n}\n",
        );

        let result = compile_to_out_dir(&src, &out)
            .expect("pipeline should compile markdown with code-behind");

        assert!(
            result.generated_routes.iter().any(|r| r.pattern == "/blog"),
            "expected route /blog"
        );

        let tpl_path = out.join("pilcrow_templates/pages/blog.html");
        assert!(
            tpl_path.exists(),
            "transpiled template should exist as .html"
        );
        let rendered = fs::read_to_string(&tpl_path).expect("read transpiled markdown template");
        assert!(
            rendered.contains("<h1>My Blog</h1>"),
            "H1 should be rendered: {rendered}"
        );

        let page = result
            .preprocessed_files
            .iter()
            .find(|f| f.module_name == "page_blog")
            .expect("page_blog module should exist");
        assert!(
            page.rust_frontmatter.contains("pub struct Props"),
            "code-behind Props should be present"
        );

        cleanup(&root);
    }

    fn mk_temp_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pilcrow_routekit_pipeline_{}_{}_{}",
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
    fn compile_pipeline_auto_layout_wraps_page_template() {
        let root = mk_temp_root("auto_layout_wrap");
        let src = root.join("src");
        let out = root.join("out");

        // A root-level _layout.html auto-layout in pages/
        write_file(
            &src.join("pages/_layout.html"),
            r#"---
pub struct Props {}
---
<div class="root-layout"><slot /></div>"#,
        );
        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");

        let result = compile_to_out_dir(&src, &out).expect("auto-layout pipeline should compile");

        // index.html should be wrapped by the auto-layout
        let page_template = out.join("pilcrow_templates/pages/index.html");
        let rendered = fs::read_to_string(page_template).expect("read page template");
        assert!(
            rendered.contains("<div class=\"root-layout\">"),
            "page should be wrapped by auto-layout"
        );
        assert!(
            rendered.contains("<h1>Home</h1>"),
            "original content preserved"
        );

        // _layout.html should NOT appear in route list
        assert!(
            result
                .generated_routes
                .iter()
                .all(|r| r.pattern != "/_layout"),
            "_layout.html should not be a route"
        );

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_auto_layout_nested_wrapping() {
        let root = mk_temp_root("auto_layout_nested");
        let src = root.join("src");
        let out = root.join("out");

        // Outer auto-layout at pages root
        write_file(
            &src.join("pages/_layout.html"),
            r#"---
pub struct Props {}
---
<html><slot /></html>"#,
        );
        // Inner auto-layout in products/
        write_file(
            &src.join("pages/products/_layout.html"),
            r#"---
pub struct Props {}
---
<section class="products"><slot /></section>"#,
        );
        write_file(&src.join("pages/products/index.html"), "<h1>Products</h1>");
        // A page NOT under products/ should only get the root layout
        write_file(&src.join("pages/about.html"), "<h1>About</h1>");

        let result = compile_to_out_dir(&src, &out).expect("nested auto-layout should compile");

        let products_template = out.join("pilcrow_templates/pages/products/index.html");
        let products_rendered = fs::read_to_string(products_template).expect("read products");
        assert!(products_rendered.contains("<html>"), "outer layout applied");
        assert!(
            products_rendered.contains("<section class=\"products\">"),
            "inner layout applied"
        );
        assert!(
            products_rendered.contains("<h1>Products</h1>"),
            "content preserved"
        );

        let about_template = out.join("pilcrow_templates/pages/about.html");
        let about_rendered = fs::read_to_string(about_template).expect("read about");
        assert!(
            about_rendered.contains("<html>"),
            "root layout applied to about"
        );
        assert!(
            !about_rendered.contains("<section class=\"products\">"),
            "products layout NOT applied to about"
        );

        // Neither _layout.html should be a route
        assert!(
            result
                .generated_routes
                .iter()
                .all(|r| !r.pattern.contains("_layout"))
        );

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_auto_layout_chain_info_recorded() {
        let root = mk_temp_root("auto_layout_chain_info");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/_layout.html"),
            r#"---
pub struct Props {
    pub site_name: String,
}
pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props { site_name: "MySite".to_string() })
}
---
<html><body><slot /></body></html>"#,
        );
        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");

        let result = compile_to_out_dir(&src, &out).expect("chain info should compile");

        // The generated templates module should include __MergedProps for the page
        let templates_src =
            fs::read_to_string(out.join("generated_templates.rs")).expect("read templates");
        assert!(
            templates_src.contains("__MergedProps"),
            "page with auto-layout load() should get __MergedProps"
        );
        assert!(
            templates_src.contains("site_name"),
            "__MergedProps should have layout's site_name field"
        );

        // _layout.html is not a route
        assert!(
            result
                .generated_routes
                .iter()
                .all(|r| r.pattern != "/_layout")
        );

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_layout_none_skips_layout_wrapping() {
        let root = mk_temp_root("layout_none");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/_layout.html"),
            r#"---
pub struct Props {
    pub site_name: String,
}
pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props { site_name: "MySite".to_string() })
}
---
<html><body><slot /></body></html>"#,
        );
        // Normal page — gets wrapped.
        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        // Opt-out page — no layout wrapping.
        write_file(
            &src.join("pages/standalone.rs"),
            r#"pub const LAYOUT: &str = "none";
pub struct Props {}
pub async fn load(_req: Req) -> AppResult<Props> { Ok(Props {}) }"#,
        );
        write_file(&src.join("pages/standalone.html"), "<h1>Standalone</h1>");

        let result = compile_to_out_dir(&src, &out).expect("should compile");

        let templates_src =
            fs::read_to_string(out.join("generated_templates.rs")).expect("read templates");

        // Normal index page should get __MergedProps with layout's site_name.
        let index_mod_start = templates_src
            .find("pub mod page_index")
            .expect("page_index mod");
        let index_mod_end = templates_src[index_mod_start..]
            .find("\npub mod ")
            .map(|p| index_mod_start + p)
            .unwrap_or(templates_src.len());
        let index_mod = &templates_src[index_mod_start..index_mod_end];
        assert!(
            index_mod.contains("__MergedProps"),
            "index page should have __MergedProps"
        );
        assert!(
            index_mod.contains("site_name"),
            "index page __MergedProps should have site_name"
        );

        // Standalone page should NOT get __MergedProps (no layout chain).
        let standalone_mod_start = templates_src
            .find("pub mod page_standalone")
            .expect("page_standalone mod");
        let standalone_mod_end = templates_src[standalone_mod_start..]
            .find("\npub mod ")
            .map(|p| standalone_mod_start + p)
            .unwrap_or(templates_src.len());
        let standalone_mod = &templates_src[standalone_mod_start..standalone_mod_end];
        assert!(
            !standalone_mod.contains("__MergedProps"),
            "standalone page with LAYOUT=none should NOT have __MergedProps; got:\n{standalone_mod}"
        );
        assert!(
            !standalone_mod.contains("site_name"),
            "standalone page should not inherit layout fields"
        );

        // Both pages should be routable.
        let patterns: Vec<_> = result
            .generated_routes
            .iter()
            .map(|r| r.pattern.as_str())
            .collect();
        assert!(patterns.contains(&"/"), "index route present");
        assert!(
            patterns.contains(&"/standalone"),
            "standalone route present"
        );

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_with_config_processes_fragment_directories() {
        use crate::templating::build_config::{FragmentEntry, PilcrowBuildConfig};

        let root = mk_temp_root("fragments_basic");
        let src = root.join("src");
        let out = root.join("out");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("widgets/user-card.html"), "<div>{{ name }}</div>");
        write_file(
            &src.join("widgets/user-card.rs"),
            "pub struct Props { pub name: String }\npub async fn load(_req: Req) -> AppResult<Props> { Ok(Props { name: \"test\".into() }) }\npub async fn refresh(_req: Req) -> ActionResult { pilcrow_web::html(\"<div>ok</div>\") }",
        );
        write_file(&src.join("partials/nav.html"), "<nav>Navigation</nav>");

        let config = PilcrowBuildConfig {
            fragments: vec![
                FragmentEntry {
                    dir: "widgets".to_string(),
                    url: None,
                },
                FragmentEntry {
                    dir: "partials".to_string(),
                    url: None,
                },
            ],
            ..Default::default()
        };

        let result =
            compile_to_out_dir_with_config(&src, &out, &config).expect("fragments should compile");

        // Pages route still present
        let patterns: Vec<_> = result
            .generated_routes
            .iter()
            .map(|r| r.pattern.as_str())
            .collect();
        assert!(patterns.contains(&"/"));

        // Fragment templates were written
        let widget_tpl = out.join("pilcrow_templates/fragments/widgets/user-card.html");
        assert!(widget_tpl.exists(), "widget template written");
        let widget_html = fs::read_to_string(&widget_tpl).expect("read widget template");
        assert!(
            widget_html.contains("{{ name }}"),
            "template content preserved"
        );

        let nav_tpl = out.join("pilcrow_templates/fragments/partials/nav.html");
        assert!(nav_tpl.exists(), "nav template written");

        // Fragment modules appear in preprocessed_files
        let frag_mods: Vec<_> = result
            .preprocessed_files
            .iter()
            .filter(|f| f.module_name.starts_with("frag_"))
            .collect();
        assert_eq!(
            frag_mods.len(),
            2,
            "two fragment modules: user-card and nav"
        );

        let widget_mod = frag_mods
            .iter()
            .find(|f| f.module_name == "frag_widgets_user_card");
        assert!(
            widget_mod.is_some(),
            "frag_widgets_user_card module present"
        );

        let app = fs::read_to_string(out.join("generated_app.rs")).expect("read generated app");
        assert!(
            app.contains(".route(\"/widgets/user-card\", ::pilcrow_web::axum::routing::post"),
            "fragment action POST route emitted"
        );
        assert!(
            app.contains(
                "\"refresh\" => match __pilcrow_gen::frag_widgets_user_card::refresh(req).await"
            ),
            "fragment action dispatches to code-behind"
        );

        let nav_mod = frag_mods
            .iter()
            .find(|f| f.module_name == "frag_partials_nav");
        assert!(nav_mod.is_some(), "frag_partials_nav module present");

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_rejects_invalid_fragment_action_signature() {
        use crate::templating::build_config::{FragmentEntry, PilcrowBuildConfig};

        let root = mk_temp_root("fragment_invalid_action");
        let src = root.join("src");
        let out = root.join("out");

        write_file(&src.join("widgets/user-card.html"), "<div>{{ name }}</div>");
        write_file(
            &src.join("widgets/user-card.rs"),
            "pub struct Props { pub name: String }\npub async fn load(_req: Req) -> AppResult<Props> { Ok(Props { name: \"test\".into() }) }\npub fn refresh(_req: Req) -> ActionResult { todo!() }",
        );

        let config = PilcrowBuildConfig {
            fragments: vec![FragmentEntry {
                dir: "widgets".to_string(),
                url: None,
            }],
            ..Default::default()
        };

        let err = compile_to_out_dir_with_config(&src, &out, &config)
            .expect_err("invalid fragment action should fail");
        assert!(err.to_string().contains("action `refresh`"));
        assert!(err.to_string().contains("must be declared `async`"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_fragment_url_prefix_override() {
        use crate::templating::build_config::{FragmentEntry, PilcrowBuildConfig};

        let root = mk_temp_root("fragment_url_override");
        let src = root.join("src");
        let out = root.join("out");

        write_file(&src.join("ui-blocks/card.html"), "<div>Card</div>");

        let config = PilcrowBuildConfig {
            fragments: vec![FragmentEntry {
                dir: "ui-blocks".to_string(),
                url: Some("blocks".to_string()),
            }],
            ..Default::default()
        };

        let result = compile_to_out_dir_with_config(&src, &out, &config)
            .expect("url-override fragments should compile");

        let block_mod = result
            .preprocessed_files
            .iter()
            .find(|f| f.module_name.starts_with("frag_blocks"));
        assert!(
            block_mod.is_some(),
            "frag_blocks_card module present with overridden prefix"
        );

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_excludes_configured_fragments_inside_pages() {
        use crate::templating::build_config::{FragmentEntry, PilcrowBuildConfig};

        let root = mk_temp_root("fragments_inside_pages");
        let out = root.join("out");

        write_file(&root.join("pages/products/index.html"), "<h1>Products</h1>");
        write_file(
            &root.join("pages/products/fragments/row.html"),
            "<tr><td>{{ name }}</td></tr>",
        );
        write_file(
            &root.join("pages/products/fragments/row.rs"),
            "pub struct Props { pub name: String }\npub async fn load(_req: Req) -> AppResult<Props> { Ok(Props { name: \"row\".into() }) }",
        );
        write_file(
            &root.join("pages/products/fragments/_error.html"),
            "<p>Fragment failed: {{ message }}</p>",
        );
        write_file(
            &root.join("pages/products/fragments/_loading.html"),
            "<p>Loading fragment</p>",
        );

        let config = PilcrowBuildConfig {
            fragments: vec![FragmentEntry {
                dir: "pages/products/fragments".to_string(),
                url: Some("products/fragments".to_string()),
            }],
            ..Default::default()
        };

        let result = compile_to_out_dir_with_config(&root, &out, &config)
            .expect("fragment inside pages should compile");
        let patterns = result
            .generated_routes
            .iter()
            .map(|route| route.pattern.as_str())
            .collect::<Vec<_>>();

        assert!(patterns.contains(&"/products"));
        assert!(
            !patterns.contains(&"/products/fragments/row"),
            "fragment file must not become a page route"
        );

        let app = fs::read_to_string(out.join("generated_app.rs")).expect("read generated app");
        assert!(app.contains("\"/products/fragments/row\""));
        assert!(app.contains("error_pages_products_fragments"));
        assert!(app.contains("loading_pages_products_fragments"));

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_expands_fragment_dir_globs() {
        use crate::templating::build_config::{FragmentEntry, PilcrowBuildConfig};

        let root = mk_temp_root("fragment_globs");
        let out = root.join("out");

        write_file(
            &root.join("pages/products/fragments/row.html"),
            "<p>Product</p>",
        );
        write_file(&root.join("pages/admin/fragments/row.html"), "<p>Admin</p>");

        let config = PilcrowBuildConfig {
            fragments: vec![FragmentEntry {
                dir: "pages/**/fragments".to_string(),
                url: Some("fragments".to_string()),
            }],
            ..Default::default()
        };

        let result = compile_to_out_dir_with_config(&root, &out, &config)
            .expect("fragment globs should compile");
        let fragment_count = result
            .preprocessed_files
            .iter()
            .filter(|file| file.module_name.starts_with("frag_fragments"))
            .count();
        assert_eq!(fragment_count, 2);

        cleanup(&root);
    }

    #[test]
    fn compile_pipeline_imports_from_ignored_relative_directory() {
        let root = mk_temp_root("ignored_relative_import");
        let out = root.join("out");

        write_file(
            &root.join("pages/products/index.html"),
            r#"---
import FilterPanel from "./components/FilterPanel.html";
---
<FilterPanel />"#,
        );
        write_file(
            &root.join("pages/products/components/FilterPanel.html"),
            r#"---
pub struct Props {}
---
<aside>Filters</aside>"#,
        );

        let config = PilcrowBuildConfig {
            routing: crate::templating::build_config::RoutingConfig {
                ignore_directories: vec!["components".to_string()],
            },
            ..Default::default()
        };

        let result = compile_to_out_dir_with_config(&root, &out, &config)
            .expect("ignored relative import should compile");
        let patterns = result
            .generated_routes
            .iter()
            .map(|route| route.pattern.as_str())
            .collect::<Vec<_>>();
        assert!(patterns.contains(&"/products"));
        assert!(!patterns.contains(&"/products/components/FilterPanel"));

        cleanup(&root);
    }

    // ── <pilcrow:head> slot expansion ────────────────────────────────────────

    #[test]
    fn compile_pipeline_pilcrow_head_hoisted_into_layout_slot() {
        let root = mk_temp_root("pilcrow_head_slot");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/_layout.html"),
            r#"---
pub struct Props {}
---
<html><head><slot name="pilcrow_head"><title>Default</title></slot></head><body><slot /></body></html>"#,
        );
        write_file(
            &src.join("pages/index.html"),
            r#"---
pub struct Props {}
---
<pilcrow:head>
<title>My Page</title>
<meta name="description" content="Great page" />
</pilcrow:head>
<h1>Hello</h1>"#,
        );

        let _result = compile_to_out_dir(&src, &out).expect("pipeline should compile");
        let page_template = out.join("pilcrow_templates/pages/index.html");
        let rendered = fs::read_to_string(page_template).expect("read transpiled page");

        assert!(
            rendered.contains("<title>My Page</title>"),
            "custom title hoisted"
        );
        assert!(
            rendered.contains(r#"<meta name="description" content="Great page" />"#),
            "meta tag hoisted"
        );
        assert!(!rendered.contains("Default"), "fallback title replaced");
        assert!(
            rendered.contains("<h1>Hello</h1>"),
            "page body in default slot"
        );
        assert!(
            !rendered.contains("pilcrow:head"),
            "pilcrow:head tag consumed"
        );
    }

    #[test]
    fn compile_pipeline_pilcrow_head_fallback_when_absent() {
        let root = mk_temp_root("pilcrow_head_fallback");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/_layout.html"),
            r#"---
pub struct Props {}
---
<html><head><slot name="pilcrow_head"><title>Site Title</title></slot></head><body><slot /></body></html>"#,
        );
        write_file(
            &src.join("pages/index.html"),
            r#"---
pub struct Props {}
---
<h1>No custom head</h1>"#,
        );

        let _result = compile_to_out_dir(&src, &out).expect("pipeline should compile");
        let page_template = out.join("pilcrow_templates/pages/index.html");
        let rendered = fs::read_to_string(page_template).expect("read transpiled page");

        assert!(
            rendered.contains("<title>Site Title</title>"),
            "fallback title used"
        );
        assert!(
            rendered.contains("<h1>No custom head</h1>"),
            "page body present"
        );
    }

    #[test]
    fn compile_pipeline_pilcrow_head_stripped_on_page_without_layout() {
        let root = mk_temp_root("pilcrow_head_no_layout");
        let src = root.join("src");
        let out = root.join("out");

        write_file(
            &src.join("pages/index.html"),
            r#"---
pub struct Props {}
pub const LAYOUT: &str = "none";
---
<pilcrow:head><title>Gone</title></pilcrow:head>
<p>Body only</p>"#,
        );

        let _result = compile_to_out_dir(&src, &out).expect("pipeline should compile");
        let page_template = out.join("pilcrow_templates/pages/index.html");
        let rendered = fs::read_to_string(page_template).expect("read transpiled page");

        assert!(!rendered.contains("pilcrow:head"), "pilcrow:head stripped");
        assert!(
            !rendered.contains("<title>Gone</title>"),
            "head content stripped"
        );
        assert!(rendered.contains("<p>Body only</p>"), "body preserved");
    }
}

#[cfg(test)]
mod nav_marker_tests {
    use super::inject_ps_nav_markers;

    #[test]
    fn nested_layout_markers_process_inside_out() {
        let input = "__PS_LAYOUT_OPEN__/__OUTER__PS_LAYOUT_OPEN__/tickets__INNER__PS_LAYOUT_CLOSE____PS_LAYOUT_CLOSE__";
        let result = inject_ps_nav_markers(input, None);
        assert!(result.contains(r#"data-ps-layout="/""#), "outer missing: {result}");
        assert!(result.contains(r#"data-ps-layout="/tickets""#), "inner missing: {result}");
        // Inner must be nested inside outer
        let outer_start = result.find(r#"data-ps-layout="/""#).unwrap();
        let inner_start = result.find(r#"data-ps-layout="/tickets""#).unwrap();
        assert!(inner_start > outer_start, "inner should be nested inside outer: {result}");
    }
}
