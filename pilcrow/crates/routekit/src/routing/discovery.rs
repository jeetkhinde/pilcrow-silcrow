use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::Route;

/// A compiled globset for ignoring directories
#[derive(Clone)]
pub struct IgnoreFilter {
    set: GlobSet,
}

impl IgnoreFilter {
    pub fn new(patterns: &[String]) -> Self {
        let mut builder = GlobSetBuilder::new();
        for p in patterns {
            let pat = if !p.contains('/') {
                format!("**/{p}")
            } else {
                p.clone()
            };
            if let Ok(glob) = Glob::new(&pat) {
                builder.add(glob);
            }
        }
        Self {
            set: builder
                .build()
                .unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap()),
        }
    }

    pub fn is_ignored(&self, relative_path: &Path) -> bool {
        self.set.is_match(relative_path)
    }
}

/// One discovered API route source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApiRoute {
    /// URL pattern, e.g. `"/api/todos"` or `"/api/users/:id"`.
    pub pattern: String,
    /// Informational Rust module path, e.g. `"api::todos"`.
    pub module_path: String,
    /// Generated symbol prefix, e.g. `"api_todos"`.
    pub symbol: String,
}

/// Discovered `.html` sources using Pilcrow's Astro-like folder convention.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveredHtmlFiles {
    pub pages: Vec<PathBuf>,
    pub ui: Vec<PathBuf>,
    /// Auto-layouts: `_layout.html` files found inside `pages/` subdirectories.
    /// These are NOT page routes — they automatically wrap sibling and child pages.
    pub auto_layouts: Vec<PathBuf>,
    /// Error boundaries: `_error.html` files found inside `pages/` subdirectories.
    /// Rendered when a page's `load()` returns `Err`. Not routable.
    pub error_pages: Vec<PathBuf>,
    /// Not-found pages: `_not_found.html` files found inside `pages/`.
    /// Rendered for unmatched routes. Not routable.
    pub not_found_pages: Vec<PathBuf>,
    /// Loading skeletons: `_loading.html` files found inside `pages/` subdirectories.
    /// Their rendered HTML is embedded as a `<template>` in sibling/descendant pages
    /// so silcrow.js can show them immediately while a navigation is in-flight.
    pub loading_pages: Vec<PathBuf>,
}

/// Discover all `.html` files from `pages` and `ui`.
///
/// Special files inside `pages/` are separated from routable pages:
/// - `_layout.html`    → `auto_layouts`
/// - `_error.html`     → `error_pages`
/// - `_not_found.html` → `not_found_pages`
///
/// `src_root` should point to the project root.
pub fn discover_html_files(
    src_root: impl AsRef<Path>,
    ignored_dirs: &[String],
) -> io::Result<DiscoveredHtmlFiles> {
    discover_html_files_with_fragment_dirs(src_root, ignored_dirs, &[])
}

pub fn discover_html_files_with_fragment_dirs(
    src_root: impl AsRef<Path>,
    ignored_dirs: &[String],
    fragment_dirs: &[PathBuf],
) -> io::Result<DiscoveredHtmlFiles> {
    let src_root = src_root.as_ref();
    let ignored = ignored_with_fragment_dirs(src_root, ignored_dirs, fragment_dirs);
    let filter = IgnoreFilter::new(&ignored);

    let all_page_files = collect_html_files(&src_root.join("pages"), src_root, &filter)?;

    let mut pages = Vec::new();
    let mut auto_layouts = Vec::new();
    let mut error_pages = Vec::new();
    let mut not_found_pages = Vec::new();

    let mut loading_pages = Vec::new();

    for path in all_page_files {
        if is_auto_layout_file(&path) {
            auto_layouts.push(path);
        } else if is_error_page_file(&path) {
            error_pages.push(path);
        } else if is_not_found_page_file(&path) {
            not_found_pages.push(path);
        } else if is_loading_page_file(&path) {
            loading_pages.push(path);
        } else {
            pages.push(path);
        }
    }

    let mut ui = collect_html_files(&src_root.join("ui"), src_root, &filter)?;

    pages.sort();
    ui.sort();
    auto_layouts.sort();
    error_pages.sort();
    not_found_pages.sort();
    loading_pages.sort();

    Ok(DiscoveredHtmlFiles {
        pages,
        ui,
        auto_layouts,
        error_pages,
        not_found_pages,
        loading_pages,
    })
}

/// Returns `true` if the file is a `_layout.html` auto-layout inside `pages/`.
fn is_auto_layout_file(path: &Path) -> bool {
    filename_is(path, "_layout.html")
}

/// Returns `true` if the file is a `_error.html` error boundary inside `pages/`.
fn is_error_page_file(path: &Path) -> bool {
    filename_is(path, "_error.html")
}

/// Returns `true` if the file is a `_not_found.html` not-found page inside `pages/`.
fn is_not_found_page_file(path: &Path) -> bool {
    filename_is(path, "_not_found.html")
}

/// Returns `true` if the file is a `_loading.html` loading skeleton inside `pages/`.
fn is_loading_page_file(path: &Path) -> bool {
    filename_is(path, "_loading.html")
}

fn filename_is(path: &Path, name: &str) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == name)
}

/// Returns `true` if the file is a special framework file (not a routable page).
fn is_special_page_file(path: &Path) -> bool {
    is_auto_layout_file(path)
        || is_error_page_file(path)
        || is_not_found_page_file(path)
        || is_loading_page_file(path)
}

/// Build `Route` entries from discovered page files in `pages`.
pub fn build_page_routes(
    src_root: impl AsRef<Path>,
    ignored_dirs: &[String],
) -> io::Result<Vec<Route>> {
    build_page_routes_with_fragment_dirs(src_root, ignored_dirs, &[])
}

pub fn build_page_routes_with_fragment_dirs(
    src_root: impl AsRef<Path>,
    ignored_dirs: &[String],
    fragment_dirs: &[PathBuf],
) -> io::Result<Vec<Route>> {
    let src_root = src_root.as_ref();
    let pages_dir = src_root.join("pages");
    let ignored = ignored_with_fragment_dirs(src_root, ignored_dirs, fragment_dirs);
    let filter = IgnoreFilter::new(&ignored);
    let mut page_files = collect_html_files(&pages_dir, src_root, &filter)?;
    // Special files (_layout, _error, _not_found) are not routable pages.
    page_files.retain(|p| !is_special_page_file(p));
    page_files.sort();

    let pages_dir_text = path_to_unix_slashes(&pages_dir);
    let mut routes = page_files
        .into_iter()
        .map(|path| {
            let file_path = path_to_unix_slashes(&path);
            Route::from_path(&file_path, &pages_dir_text)
        })
        .collect::<Vec<_>>();

    routes.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.pattern.cmp(&b.pattern))
            .then_with(|| a.template_path.cmp(&b.template_path))
    });

    Ok(routes)
}

/// Collect HTML fragment files from a fragment directory.
///
/// Returns `(path, url_prefix)` pairs where `url_prefix` is the configured prefix
/// (e.g. `"widgets"`). Special files (`_error.html`, `_layout.html`) are included
/// in their own lists so the pipeline can handle them separately.
#[derive(Debug, Clone, Default)]
pub struct DiscoveredFragmentFiles {
    /// Routable fragment HTML files.
    pub fragments: Vec<PathBuf>,
    /// `_error.html` files within this fragment group (for error rendering).
    pub error_pages: Vec<PathBuf>,
    /// `_layout.html` files within this fragment group (optional fragment wrapper).
    pub auto_layouts: Vec<PathBuf>,
    /// `_loading.html` files within this fragment group.
    pub loading_pages: Vec<PathBuf>,
}

pub fn discover_fragment_files(
    src_root: impl AsRef<Path>,
    fragment_dir: &Path,
    ignored_dirs: &[String],
) -> io::Result<DiscoveredFragmentFiles> {
    let src_root = src_root.as_ref();
    let mut result = DiscoveredFragmentFiles::default();
    if !fragment_dir.exists() {
        return Ok(result);
    }
    let filter = IgnoreFilter::new(ignored_dirs);

    let all = collect_html_files(fragment_dir, src_root, &filter)?;
    for path in all {
        if is_error_page_file(&path) {
            result.error_pages.push(path);
        } else if is_auto_layout_file(&path) {
            result.auto_layouts.push(path);
        } else if is_loading_page_file(&path) {
            result.loading_pages.push(path);
        } else {
            result.fragments.push(path);
        }
    }

    result.fragments.sort();
    result.error_pages.sort();
    result.auto_layouts.sort();
    result.loading_pages.sort();
    Ok(result)
}

/// Build `Route` entries from discovered fragment files.
///
/// Routes are prefixed with `/{url_prefix}/`.
pub fn build_fragment_routes(
    src_root: impl AsRef<Path>,
    fragment_dir: impl AsRef<Path>,
    url_prefix: &str,
    ignored_dirs: &[String],
) -> io::Result<Vec<Route>> {
    let src_root = src_root.as_ref();
    let fragment_dir = fragment_dir.as_ref();
    let filter = IgnoreFilter::new(ignored_dirs);
    let mut files = collect_html_files(fragment_dir, src_root, &filter)?;
    files.retain(|p| !is_special_page_file(p));
    files.sort();

    let dir_text = path_to_unix_slashes(fragment_dir);
    let mut routes = files
        .into_iter()
        .map(|path| {
            let file_path = path_to_unix_slashes(&path);
            let mut route = Route::from_path(&file_path, &dir_text);
            // Prefix the pattern with the url_prefix.
            let clean_prefix = url_prefix.trim_matches('/');
            route.pattern = if route.pattern == "/" {
                format!("/{clean_prefix}")
            } else {
                format!("/{clean_prefix}{}", route.pattern)
            };
            route
        })
        .collect::<Vec<_>>();

    routes.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.pattern.cmp(&b.pattern))
    });

    Ok(routes)
}

/// Discover `.rs` route files from `api/` (excludes `mod.rs`).
///
/// `src_root` should point to the project root.
#[allow(dead_code)]
pub(crate) fn discover_api_files(
    src_root: impl AsRef<Path>,
    ignored_dirs: &[String],
) -> io::Result<Vec<PathBuf>> {
    let src_root = src_root.as_ref();
    let filter = IgnoreFilter::new(ignored_dirs);
    let mut files = collect_rs_files(&src_root.join("api"), src_root, &filter)?;
    files.sort();
    Ok(files)
}

/// Build `ApiRoute` entries from `.rs` files discovered in `api/`.
pub(crate) fn build_api_routes(
    src_root: impl AsRef<Path>,
    ignored_dirs: &[String],
) -> io::Result<Vec<ApiRoute>> {
    let src_root = src_root.as_ref();
    let api_dir = src_root.join("api");
    let api_dir_text = path_to_unix_slashes(&api_dir);
    let filter = IgnoreFilter::new(ignored_dirs);

    let mut files = collect_rs_files(&api_dir, src_root, &filter)?;
    files.sort();

    let mut routes = files
        .into_iter()
        .map(|path| {
            let file_path = path_to_unix_slashes(&path);
            let relative = file_path
                .strip_prefix(&api_dir_text)
                .unwrap_or(&file_path)
                .trim_start_matches('/')
                .to_owned();
            let without_ext = relative.strip_suffix(".rs").unwrap_or(&relative).to_owned();
            let (seg_pattern, ..) = crate::routing::route::parse_pattern(&without_ext);
            let api_pattern = if seg_pattern == "/" {
                "/api".to_string()
            } else {
                format!("/api{seg_pattern}")
            };
            ApiRoute {
                pattern: api_pattern,
                module_path: build_module_path(&without_ext),
                symbol: build_api_symbol(&without_ext),
            }
        })
        .collect::<Vec<_>>();

    routes.sort_by(|a, b| a.pattern.cmp(&b.pattern));
    Ok(routes)
}

fn collect_rs_files(root: &Path, base: &Path, filter: &IgnoreFilter) -> io::Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    walk_rs_dir(root, base, filter)
}

/// Recursively collect `.rs` route files; excludes `mod.rs`.
fn walk_rs_dir(dir: &Path, base: &Path, filter: &IgnoreFilter) -> io::Result<Vec<PathBuf>> {
    fs::read_dir(dir)?.try_fold(Vec::new(), |mut acc, entry| {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let relative_path = path.strip_prefix(base).unwrap_or(&path);
            if !filter.is_ignored(relative_path) {
                acc.extend(walk_rs_dir(&path, base, filter)?);
            }
        } else if file_type.is_file() && is_rs_route(&path) {
            acc.push(path);
        }
        Ok(acc)
    })
}

fn is_rs_route(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("rs"))
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n != "mod.rs")
}

/// Build an informational Rust module path from a relative path without extension.
///
/// `users/[id]` → `"api::users::id"`
fn build_module_path(without_ext: &str) -> String {
    let parts = without_ext
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.trim_start_matches('[')
                .trim_end_matches(']')
                .trim_end_matches('?')
                .trim_start_matches("...")
        })
        .collect::<Vec<_>>();

    if parts.is_empty() {
        "api".to_string()
    } else {
        format!("api::{}", parts.join("::"))
    }
}

/// Build the `api_*` symbol name for a file path without extension.
///
/// `users/[id]` → `"api_users_id"`
fn build_api_symbol(without_ext: &str) -> String {
    let (normalized, _) =
        without_ext
            .chars()
            .fold((String::new(), false), |(mut s, prev_under), ch| {
                let mapped = if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '_'
                };
                if mapped == '_' {
                    if !prev_under {
                        s.push('_');
                    }
                    (s, true)
                } else {
                    s.push(mapped);
                    (s, false)
                }
            });

    let trimmed = normalized.trim_matches('_');
    let base = if trimmed.is_empty() { "index" } else { trimmed };
    format!("api_{base}")
}

pub fn collect_html_files_pub(
    root: &Path,
    src_root: &Path,
    ignored_dirs: &[String],
) -> io::Result<Vec<PathBuf>> {
    let filter = IgnoreFilter::new(ignored_dirs);
    collect_html_files(root, src_root, &filter)
}

fn ignored_with_fragment_dirs(
    src_root: &Path,
    ignored_dirs: &[String],
    fragment_dirs: &[PathBuf],
) -> Vec<String> {
    let mut ignored = ignored_dirs.to_vec();
    for dir in fragment_dirs {
        let rel = dir.strip_prefix(src_root).unwrap_or(dir);
        let rel = path_to_unix_slashes(rel).trim_matches('/').to_string();
        if !rel.is_empty() {
            ignored.push(rel);
        }
    }
    ignored
}

fn collect_html_files(root: &Path, base: &Path, filter: &IgnoreFilter) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    walk_dir(root, base, &mut files, filter)?;
    Ok(files)
}

fn walk_dir(
    dir: &Path,
    base: &Path,
    files: &mut Vec<PathBuf>,
    filter: &IgnoreFilter,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            let relative_path = path.strip_prefix(base).unwrap_or(&path);
            if filter.is_ignored(relative_path) {
                continue;
            }
            walk_dir(&path, base, files, filter)?;
        } else if file_type.is_file() && is_markup_file(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn is_markup_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ext.eq_ignore_ascii_case("html")
                || ext.eq_ignore_ascii_case("rhtml")
                || ext.eq_ignore_ascii_case("md")
                || ext.eq_ignore_ascii_case("mdx")
        })
}

fn path_to_unix_slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn discover_html_files_collects_by_folder_kind() {
        let root = mk_temp_root("discover_html");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/posts/[id].html"), "<h1>Post</h1>");
        write_file(&src.join("ui/Card.html"), "<div>Card</div>");
        write_file(&src.join("pages/ignore.txt"), "ignored");

        let discovered = discover_html_files(&src, &[]).expect("expected discovery to succeed");

        assert_eq!(discovered.pages.len(), 2);
        assert_eq!(discovered.ui.len(), 1);

        cleanup(&root);
    }

    #[test]
    fn discover_html_files_skips_ignored_directories() {
        let root = mk_temp_root("discover_html_ignored");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/Cards/Card.html"), "<div>Card</div>");
        write_file(&src.join("pages/posts/[id].html"), "<h1>Post</h1>");

        let ignored = vec!["Cards".to_string()];
        let discovered =
            discover_html_files(&src, &ignored).expect("expected discovery to succeed");

        assert_eq!(discovered.pages.len(), 2);
        let patterns: Vec<_> = discovered
            .pages
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
            .collect();
        assert!(patterns.contains(&"index.html"));
        assert!(patterns.contains(&"[id].html"));
        assert!(!patterns.contains(&"Card.html"));

        cleanup(&root);
    }

    #[test]
    fn discover_html_files_separates_auto_layouts() {
        let root = mk_temp_root("discover_auto_layouts");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/_layout.html"), "<slot />");
        write_file(&src.join("pages/products/index.html"), "<h1>Products</h1>");
        write_file(
            &src.join("pages/products/_layout.html"),
            "<div><slot /></div>",
        );

        let discovered = discover_html_files(&src, &[]).expect("expected discovery to succeed");

        // _layout.html files are auto_layouts, not pages
        assert_eq!(discovered.pages.len(), 2, "only non-layout pages");
        assert_eq!(discovered.auto_layouts.len(), 2, "two auto-layouts found");
        assert!(
            discovered
                .pages
                .iter()
                .all(|p| { p.file_name().and_then(|n| n.to_str()) != Some("_layout.html") }),
            "_layout.html must not appear in pages"
        );

        cleanup(&root);
    }

    #[test]
    fn discover_html_files_separates_error_and_not_found_pages() {
        let root = mk_temp_root("discover_error_nf");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/_error.html"), "<h1>Error</h1>");
        write_file(&src.join("pages/_not_found.html"), "<h1>404</h1>");
        write_file(&src.join("pages/products/index.html"), "<h1>Products</h1>");
        write_file(
            &src.join("pages/products/_error.html"),
            "<h1>Products Error</h1>",
        );

        let discovered = discover_html_files(&src, &[]).expect("discovery should succeed");

        assert_eq!(discovered.pages.len(), 2, "only routable pages");
        assert_eq!(discovered.error_pages.len(), 2, "two _error.html files");
        assert_eq!(
            discovered.not_found_pages.len(),
            1,
            "one _not_found.html file"
        );
        assert!(
            discovered.pages.iter().all(|p| {
                let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                n != "_error.html" && n != "_not_found.html"
            }),
            "special files must not appear in pages"
        );

        cleanup(&root);
    }

    #[test]
    fn build_page_routes_excludes_auto_layout_files() {
        let root = mk_temp_root("routes_exclude_auto_layout");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/_layout.html"), "<slot />");
        write_file(&src.join("pages/products/index.html"), "<h1>Products</h1>");
        write_file(
            &src.join("pages/products/_layout.html"),
            "<div><slot /></div>",
        );

        let routes = build_page_routes(&src, &[]).expect("routes should build");
        let patterns: Vec<_> = routes.iter().map(|r| r.pattern.as_str()).collect();

        assert!(patterns.contains(&"/"), "index page is a route");
        assert!(patterns.contains(&"/products"), "products page is a route");
        assert!(
            patterns.iter().all(|p| !p.contains("_layout")),
            "_layout.html files must not become routes"
        );

        cleanup(&root);
    }

    #[test]
    fn build_page_routes_excludes_error_and_not_found_files() {
        let root = mk_temp_root("routes_exclude_special");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/_error.html"), "<h1>Error</h1>");
        write_file(&src.join("pages/_not_found.html"), "<h1>404</h1>");

        let routes = build_page_routes(&src, &[]).expect("routes should build");
        let patterns: Vec<_> = routes.iter().map(|r| r.pattern.as_str()).collect();

        assert_eq!(patterns, vec!["/"], "only index.html is a route");

        cleanup(&root);
    }

    #[test]
    fn build_page_routes_maps_html_paths_to_patterns() {
        let root = mk_temp_root("build_routes");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(&src.join("pages/about.html"), "<h1>About</h1>");
        write_file(&src.join("pages/posts/[id].html"), "<h1>Post</h1>");

        let routes = build_page_routes(&src, &[]).expect("expected route manifest");
        let patterns = routes
            .iter()
            .map(|r| r.pattern.as_str())
            .collect::<Vec<_>>();

        assert!(patterns.contains(&"/"));
        assert!(patterns.contains(&"/about"));
        assert!(patterns.contains(&"/posts/:id"));

        cleanup(&root);
    }

    #[test]
    fn build_page_routes_returns_empty_when_pages_missing() {
        let root = mk_temp_root("missing_pages");
        let src = root.join("src");
        fs::create_dir_all(&src).expect("create src");

        let routes = build_page_routes(&src, &[]).expect("expected empty route list");
        assert!(routes.is_empty());

        cleanup(&root);
    }

    #[test]
    fn ignore_directories_skips_colocated_react_under_pages() {
        let root = mk_temp_root("ignore_react_pages");
        let src = root.join("src");

        write_file(&src.join("pages/index.html"), "<h1>Home</h1>");
        write_file(
            &src.join("pages/dashboard/react/Counter.html"),
            "<h1>not a route</h1>",
        );

        let routes = build_page_routes(&src, &["react".to_string()]).expect("routes build");
        let patterns = routes
            .iter()
            .map(|route| route.pattern.as_str())
            .collect::<Vec<_>>();

        assert_eq!(patterns, vec!["/"]);
        cleanup(&root);
    }

    #[test]
    fn ignore_directories_skips_colocated_react_under_fragments() {
        let root = mk_temp_root("ignore_react_fragments");
        let src = root.join("src");

        write_file(&src.join("widgets/user-card.html"), "<p>User</p>");
        write_file(
            &src.join("widgets/react/Favorite.html"),
            "<p>not a fragment</p>",
        );

        let found = discover_fragment_files(&src, &src.join("widgets"), &["react".to_string()])
            .expect("fragments discover");

        assert_eq!(found.fragments, vec![src.join("widgets/user-card.html")]);
        cleanup(&root);
    }

    fn mk_temp_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "pilcrow_routekit_{}_{}_{}",
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
    fn discover_api_files_collects_rs_files() {
        let root = mk_temp_root("discover_api");
        let src = root.join("src");

        write_file(&src.join("api/todos.rs"), "pub fn router() {}");
        write_file(&src.join("api/users/[id].rs"), "pub fn router() {}");
        write_file(&src.join("api/mod.rs"), "// ignored");
        write_file(&src.join("api/users/ignore.txt"), "ignored");

        let files = discover_api_files(&src, &[]).expect("discovery should succeed");
        assert_eq!(files.len(), 2, "mod.rs and .txt should be excluded");

        cleanup(&root);
    }

    #[test]
    fn build_api_routes_maps_rs_paths_to_patterns() {
        let root = mk_temp_root("build_api_routes");
        let src = root.join("src");

        write_file(&src.join("api/index.rs"), "pub fn router() {}");
        write_file(&src.join("api/todos.rs"), "pub fn router() {}");
        write_file(&src.join("api/users/[id].rs"), "pub fn router() {}");

        let routes = build_api_routes(&src, &[]).expect("routes should build");
        let patterns = routes
            .iter()
            .map(|r| r.pattern.as_str())
            .collect::<Vec<_>>();

        assert!(patterns.contains(&"/api"), "index.rs -> /api");
        assert!(patterns.contains(&"/api/todos"));
        assert!(patterns.contains(&"/api/users/:id"));
        assert!(routes.iter().any(|r| r.symbol == "api_todos"));
        assert!(routes.iter().any(|r| r.symbol == "api_users_id"));
        assert!(routes.iter().any(|r| r.module_path == "api::todos"));

        cleanup(&root);
    }

    #[test]
    fn build_api_routes_returns_empty_when_api_missing() {
        let root = mk_temp_root("missing_api");
        let src = root.join("src");
        fs::create_dir_all(&src).expect("create src");

        let routes = build_api_routes(&src, &[]).expect("expected empty route list");
        assert!(routes.is_empty());

        cleanup(&root);
    }

    #[test]
    fn build_api_symbol_normalizes_correctly() {
        assert_eq!(build_api_symbol("todos"), "api_todos");
        assert_eq!(build_api_symbol("users/[id]"), "api_users_id");
        assert_eq!(build_api_symbol("index"), "api_index");
        assert_eq!(build_api_symbol(""), "api_index");
    }

    #[test]
    fn build_module_path_strips_brackets() {
        assert_eq!(build_module_path("todos"), "api::todos");
        assert_eq!(build_module_path("users/[id]"), "api::users::id");
        assert_eq!(build_module_path(""), "api");
        assert_eq!(build_module_path("[...slug]"), "api::slug");
    }
}
