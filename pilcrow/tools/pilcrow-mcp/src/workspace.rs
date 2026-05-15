use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};
use syn::{FnArg, Item, ReturnType, Type, Visibility};

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FragmentGroup {
    pub dir: String,
    pub url: String,
    pub files: Vec<FileInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutDirStatus {
    pub path: Option<String>,
    pub generated_app: bool,
    pub files: Vec<FileInfo>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PropField {
    pub name: String,
    pub type_name: String,
    pub is_deferred: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodeBehindInfo {
    pub has_props: bool,
    pub prop_fields: Vec<PropField>,
    pub has_load: bool,
    pub load_is_async: bool,
    pub load_param_type: Option<String>,
    pub action_names: Vec<String>,
    pub has_deferred: bool,
    pub page_options: Vec<String>,
    pub parse_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteNode {
    pub file_path: String,
    pub url_pattern: String,
    pub is_dynamic: bool,
    pub dynamic_params: Vec<String>,
    pub has_code_behind: bool,
    pub code_behind: Option<CodeBehindInfo>,
    pub layout_chain: Vec<String>,
    pub has_loading: bool,
    pub has_error: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectContext {
    pub project_root: String,
    pub manifest_path: String,
    pub app_root: String,
    pub pilcrow_toml: Option<toml::Value>,
    pub crate_versions: BTreeMap<String, String>,
    // File lists (preserved for backward compat)
    pub routes: Vec<FileInfo>,
    pub layouts: Vec<FileInfo>,
    pub loading_skeletons: Vec<FileInfo>,
    pub not_found_pages: Vec<FileInfo>,
    pub ui_components: Vec<FileInfo>,
    pub fragments: Vec<FragmentGroup>,
    pub api_routes: Vec<FileInfo>,
    pub params: Vec<FileInfo>,
    pub middleware: Option<FileInfo>,
    pub generated_out_dir: OutDirStatus,
    // Semantic graph
    pub route_graph: Vec<RouteNode>,
    pub has_not_found_fallback: bool,
    pub has_global_error: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedProject {
    pub project_root: PathBuf,
    pub manifest_path: PathBuf,
    pub app_root: PathBuf,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct PilcrowConfig {
    #[serde(default)]
    fragments: Vec<FragmentConfig>,
    #[serde(default)]
    routing: RoutingConfig,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct RoutingConfig {
    #[serde(default)]
    ignore_directories: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct FragmentConfig {
    pub dir: String,
    pub url: Option<String>,
}

impl FragmentConfig {
    pub fn url_prefix(&self) -> String {
        self.url.clone().unwrap_or_else(|| {
            self.dir
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(&self.dir)
                .to_string()
        })
    }
}

pub fn resolve_project(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
) -> Result<ResolvedProject> {
    let root = project_root
        .map(|path| resolve_against(current_root, path))
        .unwrap_or_else(|| current_root.to_path_buf());
    let manifest = manifest_path
        .map(|path| resolve_against(&root, path))
        .unwrap_or_else(|| {
            let sandbox = root.join("sandbox/Cargo.toml");
            if sandbox.exists() {
                sandbox
            } else {
                root.join("Cargo.toml")
            }
        });

    if !manifest.exists() {
        bail!("manifest not found: {}", manifest.display());
    }
    let app_root = manifest
        .parent()
        .context("manifest path has no parent directory")?
        .to_path_buf();

    Ok(ResolvedProject {
        project_root: root,
        manifest_path: manifest,
        app_root,
    })
}

pub fn scan_project(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
) -> Result<ProjectContext> {
    let resolved = resolve_project(current_root, project_root, manifest_path)?;
    let app_src = app_source_root(&resolved.app_root);
    let pages = app_src.join("pages");

    let pilcrow_toml_path = resolved.app_root.join("Pilcrow.toml");
    let pilcrow_source = fs::read_to_string(&pilcrow_toml_path).ok();
    let pilcrow_toml = pilcrow_source
        .as_deref()
        .and_then(|source| toml::from_str::<toml::Value>(source).ok());
    let pilcrow_config = pilcrow_source
        .as_deref()
        .and_then(|source| toml::from_str::<PilcrowConfig>(source).ok())
        .unwrap_or_default();
    let ignored_dirs = pilcrow_config
        .routing
        .ignore_directories
        .iter()
        .map(|dir| normalize_path(dir))
        .collect::<Vec<_>>();

    let fragments = pilcrow_config
        .fragments
        .iter()
        .map(|entry| {
            let dir = app_src.join(&entry.dir);
            Ok(FragmentGroup {
                dir: normalize_path(&entry.dir),
                url: entry.url_prefix(),
                files: list_matching(&dir, |path| has_ext(path, "html"))?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let hooks_path = resolved.app_root.join("hooks.rs");
    let legacy_middleware_path = app_src.join("middleware.rs");
    let middleware_path = if hooks_path.exists() {
        hooks_path
    } else {
        legacy_middleware_path
    };
    let middleware = if middleware_path.exists() {
        Some(file_info(&resolved.app_root, &middleware_path)?)
    } else {
        None
    };

    let route_html_files = list_matching_ignoring(&pages, &ignored_dirs, |path| {
        has_ext(path, "html")
            && !is_special_page(path, "_layout")
            && !is_special_page(path, "_loading")
            && !is_special_page(path, "_not_found")
            && !is_special_page(path, "_error")
    })?;

    let route_graph = build_route_graph(&pages, &route_html_files);

    let has_not_found_fallback = pages.join("_not_found.html").exists();
    let has_global_error = pages.join("_error.html").exists();

    Ok(ProjectContext {
        project_root: display_path(&resolved.project_root),
        manifest_path: display_path(&resolved.manifest_path),
        app_root: display_path(&resolved.app_root),
        pilcrow_toml,
        crate_versions: crate_versions(&resolved.project_root, &resolved.manifest_path)?,
        routes: route_html_files,
        layouts: list_matching_ignoring(&pages, &ignored_dirs, |path| {
            is_special_page(path, "_layout")
        })?,
        loading_skeletons: list_matching_ignoring(&pages, &ignored_dirs, |path| {
            is_special_page(path, "_loading")
        })?,
        not_found_pages: list_matching_ignoring(&pages, &ignored_dirs, |path| {
            is_special_page(path, "_not_found") || is_special_page(path, "not-found")
        })?,
        ui_components: list_matching(&app_src.join("ui"), |path| has_ext(path, "html"))?,
        fragments,
        api_routes: list_matching(&app_src.join("api"), |path| has_ext(path, "rs"))?,
        params: list_matching(&app_src.join("params"), |path| has_ext(path, "rs"))?,
        middleware,
        generated_out_dir: out_dir_status(&resolved.manifest_path)?,
        route_graph,
        has_not_found_fallback,
        has_global_error,
    })
}

fn app_source_root(app_root: &Path) -> PathBuf {
    if app_root.join("pages").exists()
        || app_root.join("ui").exists()
        || app_root.join("api").exists()
        || app_root.join("params").exists()
    {
        app_root.to_path_buf()
    } else {
        app_root.join("src")
    }
}

fn build_route_graph(pages_root: &Path, route_files: &[FileInfo]) -> Vec<RouteNode> {
    route_files
        .iter()
        .map(|file| {
            let rel = &file.path;
            let url_pattern = derive_url_pattern(rel);
            let dynamic_params = extract_dynamic_params(&url_pattern);
            let is_dynamic = !dynamic_params.is_empty();

            let abs_html = pages_root.join(rel);
            let abs_rs = abs_html.with_extension("rs");
            let has_code_behind = abs_rs.exists();
            let code_behind = if has_code_behind {
                parse_code_behind(&abs_rs)
            } else {
                None
            };

            let layout_chain = collect_layout_chain(pages_root, rel);
            let has_loading = layout_chain_has_special(pages_root, rel, "_loading");
            let has_error = layout_chain_has_special(pages_root, rel, "_error");

            RouteNode {
                file_path: rel.clone(),
                url_pattern,
                is_dynamic,
                dynamic_params,
                has_code_behind,
                code_behind,
                layout_chain,
                has_loading,
                has_error,
            }
        })
        .collect()
}

pub fn derive_url_pattern(rel_path: &str) -> String {
    // rel_path is relative to pages root, e.g. "products/index.html" or "[id]/index.html"
    let without_ext = rel_path.trim_end_matches(".html");
    // Strip trailing /index or a bare "index"
    let stripped = if without_ext == "index" {
        ""
    } else {
        without_ext.trim_end_matches("/index")
    };

    if stripped.is_empty() {
        return "/".to_string();
    }

    let segments: Vec<&str> = stripped.split('/').collect();
    let mut url_parts = Vec::new();

    for seg in &segments {
        // Route groups: (name) -> skip
        if seg.starts_with('(') && seg.ends_with(')') {
            continue;
        }
        // Dynamic with constraint: [id=matcher] -> :id
        if seg.starts_with('[') && seg.ends_with(']') {
            let inner = &seg[1..seg.len() - 1];
            // Catch-all: [...rest]
            if inner.starts_with("...") {
                url_parts.push(format!("*{}", &inner[3..]));
            } else {
                // Strip constraints/matchers (`[id:int]`, `[id=integer]`) if present.
                let name = inner
                    .split(['=', ':'])
                    .next()
                    .unwrap_or(inner)
                    .trim_end_matches('?');
                url_parts.push(format!(":{name}"));
            }
        } else {
            url_parts.push(seg.to_string());
        }
    }

    if url_parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", url_parts.join("/"))
    }
}

fn extract_dynamic_params(url_pattern: &str) -> Vec<String> {
    url_pattern
        .split('/')
        .filter_map(|seg| {
            if seg.starts_with(':') {
                Some(seg[1..].to_string())
            } else if seg.starts_with('*') {
                Some(format!("...{}", &seg[1..]))
            } else {
                None
            }
        })
        .collect()
}

pub fn collect_layout_chain_pub(pages_root: &Path, rel_path: &str) -> Vec<String> {
    collect_layout_chain(pages_root, rel_path)
}

fn collect_layout_chain(pages_root: &Path, rel_path: &str) -> Vec<String> {
    let mut chain = Vec::new();
    // Walk from the file's directory up to pages root looking for _layout.html
    let path = Path::new(rel_path);
    let mut dir = path.parent();
    while let Some(d) = dir {
        let layout = if d.as_os_str().is_empty() {
            pages_root.join("_layout.html")
        } else {
            pages_root.join(d).join("_layout.html")
        };
        if layout.exists() {
            let rel = layout
                .strip_prefix(pages_root)
                .unwrap_or(&layout)
                .to_string_lossy()
                .into_owned();
            chain.push(rel);
        }
        dir = d.parent();
    }
    chain.reverse();
    chain
}

fn layout_chain_has_special(pages_root: &Path, rel_path: &str, special: &str) -> bool {
    let filename = format!("{special}.html");
    let path = Path::new(rel_path);
    let mut dir = path.parent();
    while let Some(d) = dir {
        let special_path = if d.as_os_str().is_empty() {
            pages_root.join(&filename)
        } else {
            pages_root.join(d).join(&filename)
        };
        if special_path.exists() {
            return true;
        }
        dir = d.parent();
    }
    false
}

pub fn parse_code_behind(path: &Path) -> Option<CodeBehindInfo> {
    let source = fs::read_to_string(path).ok()?;
    let parsed = match syn::parse_file(&source) {
        Ok(file) => file,
        Err(e) => {
            return Some(CodeBehindInfo {
                has_props: false,
                prop_fields: vec![],
                has_load: false,
                load_is_async: false,
                load_param_type: None,
                action_names: vec![],
                has_deferred: false,
                page_options: vec![],
                parse_error: Some(e.to_string()),
            });
        }
    };

    let mut has_props = false;
    let mut prop_fields = Vec::new();
    let mut has_load = false;
    let mut load_is_async = false;
    let mut load_param_type = None;
    let mut action_names = Vec::new();
    let mut page_options = Vec::new();

    for item in &parsed.items {
        match item {
            Item::Struct(s) if s.ident == "Props" => {
                has_props = true;
                if let syn::Fields::Named(named) = &s.fields {
                    for field in &named.named {
                        if let Some(ident) = &field.ident {
                            let type_name = type_to_string(&field.ty);
                            let is_deferred = type_name.starts_with("AsyncValue")
                                || type_name.starts_with("AsyncHtml");
                            prop_fields.push(PropField {
                                name: ident.to_string(),
                                type_name,
                                is_deferred,
                            });
                        }
                    }
                }
            }
            Item::Fn(f) => {
                let name = f.sig.ident.to_string();
                if name == "load" {
                    has_load = true;
                    load_is_async = f.sig.asyncness.is_some();
                    load_param_type = f.sig.inputs.iter().find_map(|arg| {
                        let FnArg::Typed(arg) = arg else { return None };
                        type_last_ident(&arg.ty)
                    });
                } else if is_action_fn(f) {
                    action_names.push(name);
                }
            }
            Item::Const(c) => {
                let name = c.ident.to_string();
                if matches!(name.as_str(), "TRAILING_SLASH" | "LAYOUT") {
                    page_options.push(name);
                }
            }
            _ => {}
        }
    }

    let has_deferred = prop_fields.iter().any(|f| f.is_deferred);

    Some(CodeBehindInfo {
        has_props,
        prop_fields,
        has_load,
        load_is_async,
        load_param_type,
        action_names,
        has_deferred,
        page_options,
        parse_error: None,
    })
}

fn is_action_fn(f: &syn::ItemFn) -> bool {
    let name = f.sig.ident.to_string();
    if name == "load" {
        return false;
    }
    if !matches!(f.vis, Visibility::Public(_)) {
        return false;
    }
    let has_req = f.sig.inputs.iter().any(|arg| {
        let FnArg::Typed(arg) = arg else { return false };
        type_contains_name(&arg.ty, "Req")
    });
    let returns_action = match &f.sig.output {
        ReturnType::Type(_, ty) => {
            type_contains_name(ty, "ActionResult") || type_contains_name(ty, "Result")
        }
        ReturnType::Default => false,
    };
    has_req && returns_action
}

fn type_to_string(ty: &Type) -> String {
    match ty {
        Type::Path(p) => {
            let segments: Vec<String> = p
                .path
                .segments
                .iter()
                .map(|seg| {
                    let ident = seg.ident.to_string();
                    match &seg.arguments {
                        syn::PathArguments::AngleBracketed(args) => {
                            let inner: Vec<String> = args
                                .args
                                .iter()
                                .map(|arg| match arg {
                                    syn::GenericArgument::Type(t) => type_to_string(t),
                                    _ => String::from("_"),
                                })
                                .collect();
                            if inner.is_empty() {
                                ident
                            } else {
                                format!("{}<{}>", ident, inner.join(", "))
                            }
                        }
                        _ => ident,
                    }
                })
                .collect();
            segments.join("::")
        }
        Type::Reference(r) => {
            let lifetime = r
                .lifetime
                .as_ref()
                .map(|l| format!("'{} ", l.ident))
                .unwrap_or_default();
            let mutability = if r.mutability.is_some() { "mut " } else { "" };
            format!("&{}{}{}", lifetime, mutability, type_to_string(&r.elem))
        }
        Type::Tuple(t) if t.elems.is_empty() => "()".to_string(),
        _ => "?".to_string(),
    }
}

fn type_contains_name(ty: &Type, expected: &str) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .iter()
            .any(|segment| segment.ident == expected),
        Type::Reference(reference) => type_contains_name(&reference.elem, expected),
        _ => false,
    }
}

fn type_last_ident(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Type::Reference(reference) => type_last_ident(&reference.elem),
        _ => None,
    }
}

pub fn find_out_dir(manifest_path: &Path) -> Result<PathBuf> {
    let manifest_dir = manifest_path
        .parent()
        .context("manifest path has no parent directory")?;
    let target_dir = find_target_dir(manifest_dir);
    let build_dir = target_dir.join("debug/build");
    if !build_dir.exists() {
        bail!("build directory not found: {}", build_dir.display());
    }

    let mut candidates = Vec::new();
    for entry in fs::read_dir(&build_dir)? {
        let entry = entry?;
        let out_dir = entry.path().join("out");
        let marker = out_dir.join("generated_app.rs");
        if marker.exists() {
            let mtime = marker
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            candidates.push((out_dir, mtime));
        }
    }
    candidates.sort_by_key(|(_, mtime)| *mtime);
    candidates
        .pop()
        .map(|(path, _)| path)
        .context("no generated_app.rs found; run codegen_build first")
}

pub fn list_files_recursive(dir: &Path) -> Result<Vec<FileInfo>> {
    list_matching(dir, |_| true)
}

pub fn ensure_contained(root: &Path, path: &Path) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let parent = path.parent().unwrap_or(root.as_path());
    let canonical_parent = if parent.exists() {
        parent.canonicalize()?
    } else {
        existing_ancestor(parent).canonicalize()?
    };
    if !canonical_parent.starts_with(&root) {
        bail!("path escapes project root: {}", path.display());
    }
    Ok(())
}

pub fn resolve_against(root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

pub fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn out_dir_status(manifest_path: &Path) -> Result<OutDirStatus> {
    match find_out_dir(manifest_path) {
        Ok(out_dir) => Ok(OutDirStatus {
            path: Some(display_path(&out_dir)),
            generated_app: out_dir.join("generated_app.rs").exists(),
            files: list_files_recursive(&out_dir)?,
            message: "generated artifacts found".to_string(),
        }),
        Err(error) => Ok(OutDirStatus {
            path: None,
            generated_app: false,
            files: vec![],
            message: error.to_string(),
        }),
    }
}

fn crate_versions(project_root: &Path, app_manifest: &Path) -> Result<BTreeMap<String, String>> {
    let mut versions = BTreeMap::new();
    let framework_root = if project_root.join("crates/web/Cargo.toml").exists() {
        project_root.to_path_buf()
    } else if project_root.join("pilcrow/crates/web/Cargo.toml").exists() {
        project_root.join("pilcrow")
    } else {
        project_root.to_path_buf()
    };
    for manifest in [
        app_manifest.to_path_buf(),
        framework_root.join("crates/web/Cargo.toml"),
        framework_root.join("crates/routekit/Cargo.toml"),
        framework_root.join("crates/runtime/Cargo.toml"),
        framework_root.join("crates/core/Cargo.toml"),
        framework_root.join("crates/client/Cargo.toml"),
        framework_root.join("crates/macros/Cargo.toml"),
    ] {
        if let Ok(source) = fs::read_to_string(&manifest) {
            if let Ok(value) = toml::from_str::<toml::Value>(&source) {
                if let Some(package) = value.get("package") {
                    let name = package.get("name").and_then(toml::Value::as_str);
                    let version = package.get("version").and_then(toml::Value::as_str);
                    if let (Some(name), Some(version)) = (name, version) {
                        versions.insert(name.to_string(), version.to_string());
                    }
                }
            }
        }
    }
    Ok(versions)
}

fn find_target_dir(manifest_dir: &Path) -> PathBuf {
    let mut dir = manifest_dir.to_path_buf();
    loop {
        let cargo_config = dir.join(".cargo/config.toml");
        if let Ok(source) = fs::read_to_string(&cargo_config) {
            if let Ok(value) = toml::from_str::<toml::Value>(&source) {
                if let Some(target_dir) = value
                    .get("build")
                    .and_then(|build| build.get("target-dir"))
                    .and_then(toml::Value::as_str)
                {
                    return dir.join(target_dir);
                }
            }
        }
        let manifest = dir.join("Cargo.toml");
        if let Ok(source) = fs::read_to_string(&manifest) {
            if source.contains("[workspace]") {
                return dir.join("target");
            }
        }
        if !dir.pop() {
            return manifest_dir.join("target");
        }
    }
}

fn list_matching(dir: &Path, predicate: impl Fn(&Path) -> bool + Copy) -> Result<Vec<FileInfo>> {
    list_matching_ignoring(dir, &[], predicate)
}

fn list_matching_ignoring(
    dir: &Path,
    ignored_dirs: &[String],
    predicate: impl Fn(&Path) -> bool + Copy,
) -> Result<Vec<FileInfo>> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }
    collect_matching(dir, dir, ignored_dirs, predicate, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_matching(
    base: &Path,
    dir: &Path,
    ignored_dirs: &[String],
    predicate: impl Fn(&Path) -> bool + Copy,
    files: &mut Vec<FileInfo>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            if is_ignored_dir(base, &path, ignored_dirs) {
                continue;
            }
            collect_matching(base, &path, ignored_dirs, predicate, files)?;
        } else if predicate(&path) {
            files.push(FileInfo {
                path: path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
                size: metadata.len(),
            });
        }
    }
    Ok(())
}

fn is_ignored_dir(base: &Path, path: &Path, ignored_dirs: &[String]) -> bool {
    let rel = path.strip_prefix(base).unwrap_or(path);
    let rel = normalize_path(&rel.to_string_lossy());
    ignored_dirs
        .iter()
        .any(|ignored| rel == *ignored || rel.starts_with(&format!("{ignored}/")))
}

fn file_info(base: &Path, path: &Path) -> Result<FileInfo> {
    let metadata = path.metadata()?;
    Ok(FileInfo {
        path: path
            .strip_prefix(base)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned(),
        size: metadata.len(),
    })
}

fn has_ext(path: &Path, ext: &str) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some(ext)
}

fn is_special_page(path: &Path, stem: &str) -> bool {
    path.file_stem().and_then(|value| value.to_str()) == Some(stem)
}

fn normalize_path(path: &str) -> String {
    path.trim_start_matches('/').to_string()
}

fn existing_ancestor(path: &Path) -> &Path {
    let mut current = path;
    while !current.exists() {
        current = current.parent().unwrap_or_else(|| Path::new("/"));
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox_workspace_root() -> PathBuf {
        let pilcrow_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let integration_root = pilcrow_root
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("workspaces/pilcrow-silcrow");
        if integration_root.join("sandbox/Cargo.toml").exists() {
            integration_root
        } else {
            pilcrow_root
        }
    }

    #[test]
    fn default_scan_finds_sandbox_routes_and_versions() {
        let root = sandbox_workspace_root();
        let context = scan_project(&root, None, None).unwrap();
        assert!(context.crate_versions.contains_key("pilcrow-web"));
        assert!(context
            .routes
            .iter()
            .any(|route| route.path == "index.html"));
    }

    #[test]
    fn containment_rejects_parent_escape() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let outside = root.join("../outside.txt");
        assert!(ensure_contained(root, &outside).is_err());
    }

    #[test]
    fn derive_url_pattern_index() {
        assert_eq!(derive_url_pattern("index.html"), "/");
    }

    #[test]
    fn derive_url_pattern_nested() {
        assert_eq!(derive_url_pattern("products/index.html"), "/products");
    }

    #[test]
    fn derive_url_pattern_dynamic() {
        assert_eq!(derive_url_pattern("[id]/index.html"), "/:id");
    }

    #[test]
    fn derive_url_pattern_constraint() {
        assert_eq!(derive_url_pattern("[id=integer]/index.html"), "/:id");
    }

    #[test]
    fn derive_url_pattern_typed_constraint() {
        assert_eq!(
            derive_url_pattern("products/[id:int].html"),
            "/products/:id"
        );
    }

    #[test]
    fn derive_url_pattern_route_group_stripped() {
        assert_eq!(derive_url_pattern("(admin)/dashboard.html"), "/dashboard");
    }

    #[test]
    fn derive_url_pattern_catch_all() {
        assert_eq!(derive_url_pattern("[...rest]/index.html"), "/*rest");
    }

    #[test]
    fn route_graph_includes_url_patterns() {
        let root = sandbox_workspace_root();
        let context = scan_project(&root, None, None).unwrap();
        let patterns: Vec<&str> = context
            .route_graph
            .iter()
            .map(|n| n.url_pattern.as_str())
            .collect();
        assert!(
            patterns.contains(&"/"),
            "expected / route, got {:?}",
            patterns
        );
    }

    #[test]
    fn code_behind_parses_load_and_actions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.rs");
        fs::write(
            &path,
            r#"
pub struct Props { pub title: String }
pub async fn load(_req: Req) -> AppResult<Props> { Ok(Props { title: "hi".into() }) }
pub async fn submit(req: Req) -> ActionResult { redirect("/") }
"#,
        )
        .unwrap();
        let info = parse_code_behind(&path).unwrap();
        assert!(info.has_load);
        assert!(info.load_is_async);
        assert_eq!(info.load_param_type.as_deref(), Some("Req"));
        assert!(info.action_names.contains(&"submit".to_string()));
        assert!(info.has_props);
    }
}
