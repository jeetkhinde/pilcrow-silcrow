use crate::workspace::{
    derive_url_pattern, parse_code_behind, resolve_project, CodeBehindInfo, RouteNode,
};
use anyhow::{bail, Result};
use serde::Serialize;
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize)]
pub struct RouteInspection {
    pub route: String,
    pub url_pattern: String,
    pub html_file: Option<String>,
    pub code_behind_file: Option<String>,
    pub template: Option<TemplateInspection>,
    pub code_behind: Option<CodeBehindInfo>,
    pub layout_chain: Vec<String>,
    pub has_loading: bool,
    pub has_error: bool,
    pub generated_hint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportInfo {
    pub component: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SilcrowDirective {
    pub directive: String,
    pub value: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateInspection {
    pub path: String,
    pub component_imports: Vec<ImportInfo>,
    pub slot_usages: Vec<String>,
    pub silcrow_directives: Vec<SilcrowDirective>,
    pub fragment_slots: Vec<String>,
    pub has_layout_opt_out: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectCodeBehindResult {
    pub path: String,
    pub info: Option<CodeBehindInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InspectGeneratedRouteResult {
    pub route: String,
    pub url_pattern: String,
    pub out_dir: Option<String>,
    pub generated_app_excerpt: Option<String>,
    pub generated_routes_excerpt: Option<String>,
    pub all_generated_files: Vec<String>,
}

pub fn inspect_route(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
    route: &str,
) -> Result<RouteInspection> {
    let resolved = resolve_project(current_root, project_root, manifest_path)?;
    let pages = resolved.app_root.join("pages");

    // Try to find the route html file by matching URL pattern
    let (html_abs, rel_path) = find_route_file(&pages, route)?;
    let url_pattern = derive_url_pattern(&rel_path);

    let rs_abs = html_abs.with_extension("rs");
    let html_file = Some(
        html_abs
            .strip_prefix(&resolved.app_root)
            .unwrap_or(&html_abs)
            .to_string_lossy()
            .into_owned(),
    );
    let code_behind_file = if rs_abs.exists() {
        Some(
            rs_abs
                .strip_prefix(&resolved.app_root)
                .unwrap_or(&rs_abs)
                .to_string_lossy()
                .into_owned(),
        )
    } else {
        None
    };

    let template = inspect_template_file(&html_abs, &html_file.clone().unwrap_or_default());
    let code_behind = if rs_abs.exists() {
        parse_code_behind(&rs_abs)
    } else {
        None
    };

    let layout_chain = crate::workspace::collect_layout_chain_pub(&pages, &rel_path);
    let has_loading = layout_chain_has(&pages, &rel_path, "_loading");
    let has_error = layout_chain_has(&pages, &rel_path, "_error");

    let generated_hint = format!(
        "Run codegen_build then codegen_read with file=\"generated_app.rs\" to see the wired route handler for {}.",
        url_pattern
    );

    Ok(RouteInspection {
        route: route.to_string(),
        url_pattern,
        html_file,
        code_behind_file,
        template,
        code_behind,
        layout_chain,
        has_loading,
        has_error,
        generated_hint,
    })
}

pub fn inspect_template(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
    path: &str,
) -> Result<TemplateInspection> {
    let resolved = resolve_project(current_root, project_root, manifest_path)?;
    let abs_path = if Path::new(path).is_absolute() {
        Path::new(path).to_path_buf()
    } else {
        resolved.app_root.join(path)
    };
    if !abs_path.exists() {
        bail!("template not found: {}", abs_path.display());
    }
    Ok(
        inspect_template_file(&abs_path, path).unwrap_or_else(|| TemplateInspection {
            path: path.to_string(),
            component_imports: vec![],
            slot_usages: vec![],
            silcrow_directives: vec![],
            fragment_slots: vec![],
            has_layout_opt_out: false,
        }),
    )
}

pub fn inspect_code_behind(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
    path: &str,
) -> Result<InspectCodeBehindResult> {
    let resolved = resolve_project(current_root, project_root, manifest_path)?;
    let abs_path = if Path::new(path).is_absolute() {
        Path::new(path).to_path_buf()
    } else {
        resolved.app_root.join(path)
    };
    if !abs_path.exists() {
        bail!("code-behind file not found: {}", abs_path.display());
    }
    let info = parse_code_behind(&abs_path);
    Ok(InspectCodeBehindResult {
        path: path.to_string(),
        info,
    })
}

pub fn inspect_generated_route(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
    route: &str,
) -> Result<InspectGeneratedRouteResult> {
    use crate::workspace::find_out_dir;

    let resolved = resolve_project(current_root, project_root, manifest_path)?;
    let pages = resolved.app_root.join("pages");

    let url_pattern = if let Ok((_, rel)) = find_route_file(&pages, route) {
        derive_url_pattern(&rel)
    } else {
        route.to_string()
    };

    let out_dir = match find_out_dir(&resolved.manifest_path) {
        Ok(d) => d,
        Err(_) => {
            return Ok(InspectGeneratedRouteResult {
                route: route.to_string(),
                url_pattern,
                out_dir: None,
                generated_app_excerpt: None,
                generated_routes_excerpt: None,
                all_generated_files: vec![],
            });
        }
    };

    let generated_app = fs::read_to_string(out_dir.join("generated_app.rs")).ok();
    let generated_routes = fs::read_to_string(out_dir.join("generated_routes.rs")).ok();

    let app_excerpt = generated_app
        .as_deref()
        .map(|src| excerpt_for_route(src, route, &url_pattern));
    let routes_excerpt = generated_routes
        .as_deref()
        .map(|src| excerpt_for_route(src, route, &url_pattern));

    let all_files = crate::workspace::list_files_recursive(&out_dir)
        .unwrap_or_default()
        .into_iter()
        .map(|f| f.path)
        .collect();

    Ok(InspectGeneratedRouteResult {
        route: route.to_string(),
        url_pattern,
        out_dir: Some(out_dir.to_string_lossy().into_owned()),
        generated_app_excerpt: app_excerpt,
        generated_routes_excerpt: routes_excerpt,
        all_generated_files: all_files,
    })
}

fn inspect_template_file(abs_path: &Path, display_path: &str) -> Option<TemplateInspection> {
    let source = fs::read_to_string(abs_path).ok()?;
    let mut component_imports = Vec::new();
    let mut slot_usages = Vec::new();
    let mut silcrow_directives = Vec::new();
    let mut fragment_slots = Vec::new();
    let has_layout_opt_out = source.contains("LAYOUT") && source.contains("none");

    for (idx, line) in source.lines().enumerate() {
        let lnum = idx + 1;
        let trimmed = line.trim();

        // Askama-style imports: {% import "ui/Button.html" as Button %}
        if trimmed.starts_with("{%") && trimmed.contains("import") && trimmed.contains('"') {
            if let Some(path) = extract_quoted(trimmed) {
                let component = Path::new(&path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.clone());
                component_imports.push(ImportInfo { component, path });
            }
        }

        // Slot usages: {{ slot }} or <slot name="X">
        if trimmed.contains("{{ slot }}") || trimmed.contains("{{slot}}") {
            slot_usages.push("default".to_string());
        }
        if let Some(name) = extract_slot_name(trimmed) {
            slot_usages.push(name);
        }

        // Fragment slots: data-pilcrow-slot or __pilcrow_html_slot_
        if trimmed.contains("data-pilcrow-slot") || trimmed.contains("__pilcrow_html_slot_") {
            if let Some(slot) = extract_pilcrow_slot(trimmed) {
                fragment_slots.push(slot);
            }
        }

        // Silcrow directives: s-get, s-post, s-put, s-patch, s-delete, s-target, s-sse, s-ws
        for directive in &[
            "s-get", "s-post", "s-put", "s-patch", "s-delete", "s-target", "s-boost", "s-html",
            "s-sse", "s-ws", "s-for", "s-use",
        ] {
            if line.contains(directive) {
                let value = extract_directive_value(line, directive).unwrap_or_default();
                silcrow_directives.push(SilcrowDirective {
                    directive: directive.to_string(),
                    value,
                    line: lnum,
                });
            }
        }
    }

    // Deduplicate slot usages
    slot_usages.dedup();

    Some(TemplateInspection {
        path: display_path.to_string(),
        component_imports,
        slot_usages,
        silcrow_directives,
        fragment_slots,
        has_layout_opt_out,
    })
}

fn extract_quoted(s: &str) -> Option<String> {
    let start = s.find('"')? + 1;
    let end = s[start..].find('"')? + start;
    Some(s[start..end].to_string())
}

fn extract_slot_name(s: &str) -> Option<String> {
    if s.contains("<slot") && s.contains("name=") {
        let after_name = s.find("name=")? + 5;
        let rest = &s[after_name..];
        let quote_char = rest.chars().next()?;
        if quote_char == '"' || quote_char == '\'' {
            let inner = &rest[1..];
            let end = inner.find(quote_char)?;
            return Some(inner[..end].to_string());
        }
    }
    None
}

fn extract_pilcrow_slot(s: &str) -> Option<String> {
    if let Some(pos) = s.find("data-pilcrow-slot=\"") {
        let start = pos + "data-pilcrow-slot=\"".len();
        let end = s[start..].find('"')? + start;
        return Some(s[start..end].to_string());
    }
    None
}

fn extract_directive_value(line: &str, directive: &str) -> Option<String> {
    let pos = line.find(directive)? + directive.len();
    let rest = line[pos..].trim_start();
    if rest.starts_with('=') {
        let after_eq = rest[1..].trim_start();
        let quote = after_eq.chars().next()?;
        if quote == '"' || quote == '\'' {
            let inner = &after_eq[1..];
            let end = inner.find(quote)?;
            return Some(inner[..end].to_string());
        }
    }
    None
}

fn find_route_file(pages_root: &Path, route: &str) -> Result<(std::path::PathBuf, String)> {
    // Try to find by URL pattern match or by direct path
    let normalized = route.trim_start_matches('/').trim_end_matches('/');

    // Try as direct file path first
    let candidates = [
        format!("{}.html", normalized),
        format!("{}/index.html", normalized),
        if normalized.is_empty() {
            "index.html".to_string()
        } else {
            format!("{normalized}/index.html")
        },
    ];

    for candidate in &candidates {
        let abs = pages_root.join(candidate);
        if abs.exists() {
            return Ok((abs, candidate.clone()));
        }
    }

    // Fallback: scan all pages and match by URL pattern
    let mut matches = Vec::new();
    let target = if normalized.is_empty() {
        "/".to_string()
    } else {
        format!("/{normalized}")
    };

    collect_route_matches(pages_root, pages_root, &target, &mut matches);

    matches
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("route not found: {route}"))
}

fn collect_route_matches(
    base: &Path,
    dir: &Path,
    target: &str,
    matches: &mut Vec<(std::path::PathBuf, String)>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_route_matches(base, &path, target, matches);
        } else if path.extension().and_then(|e| e.to_str()) == Some("html") {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if matches!(stem, "_layout" | "_loading" | "_error" | "_not_found") {
                continue;
            }
            let pattern = derive_url_pattern(&rel);
            if url_patterns_match(&pattern, target) {
                matches.push((path, rel));
            }
        }
    }
}

fn url_patterns_match(pattern: &str, target: &str) -> bool {
    // Simple match: exact or dynamic param substitution
    if pattern == target {
        return true;
    }
    let p_segs: Vec<&str> = pattern.split('/').collect();
    let t_segs: Vec<&str> = target.split('/').collect();
    if p_segs.len() != t_segs.len() {
        return false;
    }
    p_segs
        .iter()
        .zip(t_segs.iter())
        .all(|(p, t)| p == t || p.starts_with(':') || p.starts_with('*'))
}

fn layout_chain_has(pages_root: &Path, rel_path: &str, special: &str) -> bool {
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

fn excerpt_for_route(source: &str, route: &str, url_pattern: &str) -> String {
    let search_terms = [
        route.trim_start_matches('/'),
        url_pattern.trim_start_matches('/'),
    ];
    let lines: Vec<&str> = source.lines().collect();
    let mut excerpts = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let lower = line.to_ascii_lowercase();
        if search_terms
            .iter()
            .any(|t| lower.contains(&t.to_ascii_lowercase()))
        {
            let start = idx.saturating_sub(2);
            let end = (idx + 3).min(lines.len());
            let chunk = lines[start..end].join("\n");
            if !excerpts.contains(&chunk) {
                excerpts.push(chunk);
            }
            if excerpts.len() >= 3 {
                break;
            }
        }
    }
    if excerpts.is_empty() {
        format!("No excerpt found for route '{}' in generated file.", route)
    } else {
        excerpts.join("\n---\n")
    }
}

// Make collect_layout_chain available as a pub function from workspace
impl crate::workspace::RouteNode {
    #[allow(dead_code)]
    pub fn from_file(pages_root: &Path, rel_path: &str) -> RouteNode {
        let url_pattern = derive_url_pattern(rel_path);
        let dynamic_params = url_pattern
            .split('/')
            .filter_map(|seg| {
                if seg.starts_with(':') {
                    Some(seg[1..].to_string())
                } else {
                    None
                }
            })
            .collect();
        let is_dynamic = url_pattern.contains(':') || url_pattern.contains('*');
        let abs_html = pages_root.join(rel_path);
        let abs_rs = abs_html.with_extension("rs");
        let has_code_behind = abs_rs.exists();
        let code_behind = if has_code_behind {
            parse_code_behind(&abs_rs)
        } else {
            None
        };
        let layout_chain = crate::workspace::collect_layout_chain_pub(pages_root, rel_path);
        let has_loading = layout_chain_has(pages_root, rel_path, "_loading");
        let has_error = layout_chain_has(pages_root, rel_path, "_error");
        RouteNode {
            file_path: rel_path.to_string(),
            url_pattern,
            is_dynamic,
            dynamic_params,
            has_code_behind,
            code_behind,
            layout_chain,
            has_loading,
            has_error,
        }
    }
}
