use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
struct RouteInfo {
    file_path: String,
    url_pattern: String,
    dynamic_params: Vec<String>,
    layouts: Vec<String>,
    has_loading: bool,
    has_error: bool,
    code_behind: Option<CodeBehindInfo>,
}

#[derive(Debug, Default)]
struct CodeBehindInfo {
    load: Option<String>,
    actions: Vec<String>,
    cache: Vec<String>,
    options: Vec<String>,
}

pub fn handle_routes(args: &[String]) -> Result<(), String> {
    let app_root = args
        .iter()
        .find(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let pages = app_root.join("pages");
    if !pages.exists() {
        return Err(format!(
            "expected Pilcrow pages directory at {}",
            pages.display()
        ));
    }

    let ignored_dirs = read_ignored_directories(&app_root.join("Pilcrow.toml"));
    let mut html_files = Vec::new();
    collect_routes(&pages, &pages, &ignored_dirs, &mut html_files)?;
    html_files.sort();

    let routes = html_files
        .iter()
        .map(|rel| route_info(&pages, rel))
        .collect::<Vec<_>>();

    print_routes(&app_root, &routes);
    Ok(())
}

fn collect_routes(
    root: &Path,
    dir: &Path,
    ignored_dirs: &BTreeSet<String>,
    files: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|err| format!("read {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read {}: {err}", dir.display()))?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();

        if path.is_dir() {
            if ignored_dirs.contains(&file_name) {
                continue;
            }
            collect_routes(root, &path, ignored_dirs, files)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("html")
            && !is_special_page(&path)
        {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            files.push(rel);
        }
    }
    Ok(())
}

fn route_info(pages_root: &Path, rel: &str) -> RouteInfo {
    let url_pattern = derive_url_pattern(rel);
    let dynamic_params = extract_dynamic_params(&url_pattern);
    let html_path = pages_root.join(rel);
    let rs_path = html_path.with_extension("rs");
    let code_behind = parse_code_behind(&rs_path);

    RouteInfo {
        file_path: rel.to_string(),
        url_pattern,
        dynamic_params,
        layouts: collect_layout_chain(pages_root, rel, code_behind.as_ref()),
        has_loading: has_special_in_hierarchy(pages_root, rel, "_loading"),
        has_error: has_special_in_hierarchy(pages_root, rel, "_error"),
        code_behind,
    }
}

fn print_routes(app_root: &Path, routes: &[RouteInfo]) {
    println!("Pilcrow routes for {}", app_root.display());
    println!();

    if routes.is_empty() {
        println!("No page routes found.");
        return;
    }

    println!(
        "{:<24}  {:<32}  {:<18}  {:<18}  {}",
        "URL", "Source", "Load", "Cache", "Actions"
    );
    println!("{}", "-".repeat(112));

    for route in routes {
        let load = route
            .code_behind
            .as_ref()
            .and_then(|info| info.load.as_deref())
            .unwrap_or("-");
        let cache = route
            .code_behind
            .as_ref()
            .map(|info| join_or_dash(&info.cache))
            .unwrap_or_else(|| "-".to_string());
        let actions = route
            .code_behind
            .as_ref()
            .map(|info| join_or_dash(&info.actions))
            .unwrap_or_else(|| "-".to_string());

        println!(
            "{:<24}  {:<32}  {:<18}  {:<18}  {}",
            route.url_pattern, route.file_path, load, cache, actions
        );

        let mut details = Vec::new();
        if !route.dynamic_params.is_empty() {
            details.push(format!("params: {}", route.dynamic_params.join(", ")));
        }
        if !route.layouts.is_empty() {
            details.push(format!("layouts: {}", route.layouts.join(" -> ")));
        }
        if route.has_loading {
            details.push("loading".to_string());
        }
        if route.has_error {
            details.push("error".to_string());
        }
        if let Some(info) = &route.code_behind {
            if !info.options.is_empty() {
                details.push(format!("options: {}", info.options.join(", ")));
            }
        }
        if !details.is_empty() {
            println!("  {}", details.join(" | "));
        }
    }
}

fn parse_code_behind(path: &Path) -> Option<CodeBehindInfo> {
    let source = fs::read_to_string(path).ok()?;
    let mut info = CodeBehindInfo::default();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub async fn load") || trimmed.starts_with("pub fn load") {
            let asyncness = if trimmed.starts_with("pub async fn") {
                "async"
            } else {
                "sync"
            };
            let arg = extract_first_arg_type(trimmed).unwrap_or_else(|| "?".to_string());
            info.load = Some(format!("{asyncness} {arg}"));
        } else if trimmed.starts_with("pub async fn ") || trimmed.starts_with("pub fn ") {
            if (trimmed.contains("ActionResult") || trimmed.contains("Result"))
                && trimmed.contains("Req")
            {
                if let Some(name) = extract_fn_name(trimmed) {
                    if name != "load" {
                        info.actions.push(name);
                    }
                }
            }
        } else if trimmed.starts_with("pub const ") {
            collect_page_const(trimmed, &mut info);
        }
    }

    info.actions.sort();
    Some(info)
}

fn collect_page_const(line: &str, info: &mut CodeBehindInfo) {
    let Some(name) = line
        .trim_start_matches("pub const ")
        .split([':', '='])
        .next()
        .map(str::trim)
    else {
        return;
    };

    match name {
        "REVALIDATE" | "MAX_STALE" | "CACHE_TAGS" | "CACHE_VARY" => {
            info.cache.push(short_const(line, name));
        }
        "PRERENDER" | "STREAMING" | "TRAILING_SLASH" | "LAYOUT" => {
            info.options.push(short_const(line, name));
        }
        _ => {}
    }
}

fn short_const(line: &str, name: &str) -> String {
    let value = line
        .split_once('=')
        .map(|(_, value)| value.trim().trim_end_matches(';'))
        .unwrap_or("?")
        .replace("&[", "[");
    format!("{name}={value}")
}

fn extract_fn_name(line: &str) -> Option<String> {
    let marker = "fn ";
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest.find('(')?;
    Some(rest[..end].trim().to_string())
}

fn extract_first_arg_type(line: &str) -> Option<String> {
    let args = line.split_once('(')?.1.split_once(')')?.0.trim();
    if args.is_empty() {
        return Some("()".to_string());
    }
    let first = args.split(',').next()?.trim();
    let ty = first.split_once(':')?.1.trim();
    Some(
        ty.split(|ch: char| !ch.is_alphanumeric() && ch != '_')
            .filter(|part| !part.is_empty())
            .next_back()
            .unwrap_or(ty)
            .to_string(),
    )
}

fn collect_layout_chain(
    pages_root: &Path,
    rel_path: &str,
    code_behind: Option<&CodeBehindInfo>,
) -> Vec<String> {
    if code_behind.is_some_and(|info| {
        info.options
            .iter()
            .any(|option| option == "LAYOUT=\"none\"")
    }) {
        return Vec::new();
    }

    let mut chain = Vec::new();
    let mut current = Path::new(rel_path).parent();
    while let Some(dir) = current {
        let layout = if dir.as_os_str().is_empty() {
            pages_root.join("_layout.html")
        } else {
            pages_root.join(dir).join("_layout.html")
        };
        if layout.exists() {
            chain.push(relative_to(pages_root, &layout));
        }
        current = dir.parent();
    }
    chain.reverse();
    chain
}

fn has_special_in_hierarchy(pages_root: &Path, rel_path: &str, special: &str) -> bool {
    let filename = format!("{special}.html");
    let mut current = Path::new(rel_path).parent();
    while let Some(dir) = current {
        let path = if dir.as_os_str().is_empty() {
            pages_root.join(&filename)
        } else {
            pages_root.join(dir).join(&filename)
        };
        if path.exists() {
            return true;
        }
        current = dir.parent();
    }
    false
}

fn derive_url_pattern(rel_path: &str) -> String {
    let without_ext = rel_path.trim_end_matches(".html");
    let stripped = if without_ext == "index" {
        ""
    } else {
        without_ext.trim_end_matches("/index")
    };

    if stripped.is_empty() {
        return "/".to_string();
    }

    let mut url_parts = Vec::new();
    for segment in stripped.split('/') {
        if segment.starts_with('(') && segment.ends_with(')') {
            continue;
        }
        if segment.starts_with('[') && segment.ends_with(']') {
            let inner = &segment[1..segment.len() - 1];
            if let Some(rest) = inner.strip_prefix("...") {
                url_parts.push(format!("*{rest}"));
            } else {
                let name = inner
                    .split(['=', ':'])
                    .next()
                    .unwrap_or(inner)
                    .trim_end_matches('?');
                url_parts.push(format!(":{name}"));
            }
        } else {
            url_parts.push(segment.to_string());
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
        .filter_map(|segment| {
            segment
                .strip_prefix(':')
                .map(str::to_string)
                .or_else(|| segment.strip_prefix('*').map(|name| format!("...{name}")))
        })
        .collect()
}

fn read_ignored_directories(path: &Path) -> BTreeSet<String> {
    let Ok(source) = fs::read_to_string(path) else {
        return BTreeSet::new();
    };

    let mut ignored = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("ignore_directories") {
            continue;
        }
        let Some((_, value)) = trimmed.split_once('=') else {
            continue;
        };
        for part in value
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
        {
            let dir = part.trim().trim_matches('"').trim_matches('\'');
            if !dir.is_empty() {
                ignored.insert(dir.to_string());
            }
        }
    }
    ignored
}

fn is_special_page(path: &Path) -> bool {
    matches!(
        path.file_stem().and_then(|value| value.to_str()),
        Some("_layout" | "_loading" | "_not_found" | "_error" | "not-found")
    )
}

fn join_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_route_patterns() {
        assert_eq!(derive_url_pattern("index.html"), "/");
        assert_eq!(derive_url_pattern("products/index.html"), "/products");
        assert_eq!(derive_url_pattern("(admin)/dashboard.html"), "/dashboard");
        assert_eq!(
            derive_url_pattern("products/[id:int].html"),
            "/products/:id"
        );
        assert_eq!(derive_url_pattern("[...rest]/index.html"), "/*rest");
    }

    #[test]
    fn reads_inline_ignored_directories() {
        let mut ignored = BTreeSet::new();
        ignored.insert("Cards".to_string());
        assert_eq!(
            read_ignored_directories_from_source("ignore_directories = [\"Cards\"]"),
            ignored
        );
    }

    fn read_ignored_directories_from_source(source: &str) -> BTreeSet<String> {
        let mut ignored = BTreeSet::new();
        for line in source.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("ignore_directories") {
                continue;
            }
            let Some((_, value)) = trimmed.split_once('=') else {
                continue;
            };
            for part in value
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
            {
                let dir = part.trim().trim_matches('"').trim_matches('\'');
                if !dir.is_empty() {
                    ignored.insert(dir.to_string());
                }
            }
        }
        ignored
    }
}
