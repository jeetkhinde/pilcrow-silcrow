use std::collections::HashMap;
use std::env;
use std::io;
use std::path::PathBuf;

use routing::path::{PathHierarchy, normalize_path};

pub mod fsr;
pub mod routing;
pub mod templating;

pub use routing::constraint::ParameterConstraint;
pub use routing::intercept::InterceptLevel;
pub use templating::build_config::{FragmentEntry, PilcrowBuildConfig};
pub use templating::codegen::{GeneratedApiRoute, GeneratedPageRoute};
pub use templating::layout::LayoutOption;
pub use templating::pipeline::{
    compile_to_out_dir, compile_to_out_dir_with_config, watched_source_directories,
};

pub fn compile_current_crate_sources() -> io::Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|err| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("CARGO_MANIFEST_DIR must be set: {err}"),
        )
    })?);
    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("OUT_DIR must be set: {err}"),
        )
    })?);

    let build_config = PilcrowBuildConfig::load_from(&manifest_dir);

    compile_to_out_dir_with_config(&manifest_dir, &out_dir, &build_config)?;

    for dir in watched_source_directories(&manifest_dir) {
        println!("cargo:rerun-if-changed={}", dir.display());
    }
    // Watch each configured fragment directory for changes.
    for entry in &build_config.fragments {
        let frag_dir = entry.abs_dir(&manifest_dir);
        println!("cargo:rerun-if-changed={}", frag_dir.display());
    }
    // Watch the config file itself.
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("Pilcrow.toml").display()
    );

    // Watch the locales directory when i18n is configured.
    if !build_config.i18n.locales.is_empty() {
        let locales_dir = manifest_dir.join(&build_config.i18n.locales_dir);
        println!("cargo:rerun-if-changed={}", locales_dir.display());
    }

    Ok(())
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Route {
    pub(crate) pattern: String,
    pub(crate) template_path: String,
    pub(crate) params: Vec<String>,
    pub(crate) priority: usize,
    pub(crate) is_layout: bool,
    #[allow(dead_code)]
    pub(crate) has_catch_all: bool,
    #[allow(dead_code)]
    pub(crate) optional_params: Vec<String>,
    pub(crate) is_error_page: bool,
    pub(crate) is_nolayout_marker: bool,
    pub(crate) is_loading: bool,
    pub(crate) is_template: bool,
    pub(crate) is_not_found: bool,
    pub(crate) is_parallel_route: bool,
    pub(crate) parallel_slot: Option<String>,
    pub(crate) is_intercepting: bool,
    #[allow(dead_code)]
    pub(crate) intercept_level: Option<InterceptLevel>,
    #[allow(dead_code)]
    pub(crate) intercept_target: Option<String>,
    pub(crate) layout_option: LayoutOption,
    pub(crate) layout_name: Option<String>,
    pub(crate) metadata: HashMap<String, String>,
    pub(crate) param_constraints: HashMap<String, ParameterConstraint>,
    pub(crate) aliases: Vec<String>,
    pub(crate) name: Option<String>,
    pub(crate) is_redirect: bool,
    pub(crate) redirect_to: Option<String>,
    pub(crate) redirect_status: Option<u16>,
}

#[non_exhaustive]
#[derive(Debug)]
pub struct RouteMatch<'a> {
    pub(crate) route: &'a Route,
    pub params: HashMap<String, String>,
}

impl<'a> RouteMatch<'a> {
    pub fn is_redirect(&self) -> bool {
        self.route.is_redirect
    }

    pub fn redirect_target(&self) -> Option<String> {
        self.route.redirect_target(&self.params)
    }

    pub fn redirect_status(&self) -> Option<u16> {
        self.route.redirect_status
    }
}

impl Route {
    pub fn from_path(file_path: &str, pages_dir: &str) -> Self {
        let relative = file_path
            .strip_prefix(pages_dir)
            .unwrap_or(file_path)
            .trim_start_matches('/');

        let without_ext = relative
            .strip_suffix(".rhtml")
            .or_else(|| relative.strip_suffix(".html"))
            .or_else(|| relative.strip_suffix(".md"))
            .or_else(|| relative.strip_suffix(".mdx"))
            .unwrap_or(relative);

        let filename = without_ext.split('/').next_back().unwrap_or("");

        let is_layout = filename == "_layout" || filename.starts_with("_layout.");
        let is_error_page = filename == "_error";
        let is_nolayout_marker = filename == "_nolayout";
        let is_template = filename == "_template";
        let is_not_found = filename == "not-found";

        let (is_parallel_route, parallel_slot) = routing::route::detect_parallel_route(without_ext);

        let (is_intercepting, intercept_level, intercept_target) =
            routing::route::detect_intercepting_route(without_ext);

        let layout_name = if is_layout {
            routing::route::extract_layout_name(filename)
        } else {
            None
        };

        let (pattern, params, optional_params, dynamic_count, has_catch_all, param_constraints, has_optional_catch_all) =
            routing::route::parse_pattern(without_ext);

        let depth = pattern.matches('/').count();
        let priority = routing::route::calculate_priority(
            has_catch_all,
            has_optional_catch_all,
            dynamic_count,
            depth,
            &optional_params,
        );

        Route {
            pattern,
            template_path: file_path.to_string(),
            params,
            priority,
            is_layout,
            has_catch_all,
            optional_params,
            is_error_page,
            is_nolayout_marker,
            is_loading: false,
            is_template,
            is_not_found,
            is_parallel_route,
            parallel_slot,
            is_intercepting,
            intercept_level,
            intercept_target,
            layout_option: LayoutOption::default(),
            layout_name,
            metadata: HashMap::new(),
            param_constraints,
            aliases: Vec::new(),
            name: None,
            is_redirect: false,
            redirect_to: None,
            redirect_status: None,
        }
    }

    pub fn matches(&self, path: &str) -> Option<HashMap<String, String>> {
        self.matches_with_options(path, false)
    }

    pub fn matches_with_options(
        &self,
        path: &str,
        case_insensitive: bool,
    ) -> Option<HashMap<String, String>> {
        if self.is_redirect && self.params.is_empty() {
            let matches = if case_insensitive {
                self.pattern.eq_ignore_ascii_case(path)
            } else {
                self.pattern == path
            };
            return if matches { Some(HashMap::new()) } else { None };
        }

        let pattern_segments: Vec<&str> =
            self.pattern.split('/').filter(|s| !s.is_empty()).collect();
        let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        let mut params = HashMap::new();
        let mut pattern_idx = 0;
        let mut path_idx = 0;

        while pattern_idx < pattern_segments.len() {
            let pattern_seg = pattern_segments[pattern_idx];

            match pattern_seg.chars().next() {
                Some('*') => {
                    let is_optional = pattern_seg.ends_with('?');
                    let param_name = if is_optional {
                        &pattern_seg[1..pattern_seg.len() - 1]
                    } else {
                        &pattern_seg[1..]
                    };

                    let remaining: Vec<&str> = path_segments[path_idx..].to_vec();

                    if remaining.is_empty() && !is_optional {
                        return None;
                    }

                    params.insert(param_name.to_string(), remaining.join("/"));
                    return Some(params);
                }
                Some(':') if pattern_seg.ends_with('?') => {
                    let param_name = &pattern_seg[1..pattern_seg.len() - 1];

                    if path_idx < path_segments.len() {
                        let should_consume = if pattern_idx + 1 < pattern_segments.len() {
                            let next_pattern = pattern_segments[pattern_idx + 1];
                            match next_pattern.chars().next() {
                                Some(':') | Some('*') => true,
                                _ => {
                                    if case_insensitive {
                                        !next_pattern.eq_ignore_ascii_case(path_segments[path_idx])
                                    } else {
                                        next_pattern != path_segments[path_idx]
                                    }
                                }
                            }
                        } else {
                            true
                        };

                        if should_consume && path_idx < path_segments.len() {
                            params.insert(
                                param_name.to_string(),
                                path_segments[path_idx].to_string(),
                            );
                            path_idx += 1;
                        }
                    }
                    pattern_idx += 1;
                }
                Some(':') => {
                    if path_idx >= path_segments.len() {
                        return None;
                    }
                    let param_name = &pattern_seg[1..];
                    params.insert(param_name.to_string(), path_segments[path_idx].to_string());
                    path_idx += 1;
                    pattern_idx += 1;
                }
                _ => {
                    if path_idx >= path_segments.len() {
                        return None;
                    }

                    let matches = if case_insensitive {
                        pattern_seg.eq_ignore_ascii_case(path_segments[path_idx])
                    } else {
                        pattern_seg == path_segments[path_idx]
                    };

                    if !matches {
                        return None;
                    }

                    path_idx += 1;
                    pattern_idx += 1;
                }
            }
        }

        if path_idx == path_segments.len() {
            let all_valid = params.iter().all(|(param_name, param_value)| {
                self.param_constraints
                    .get(param_name)
                    .map(|constraint| constraint.validate(param_value))
                    .unwrap_or(true) // No constraint = always valid
            });

            if all_valid {
                Some(params)
            } else {
                None // Constraint validation failed
            }
        } else {
            None
        }
    }

    pub fn layout_pattern(&self) -> Option<String> {
        if let Some(last_slash) = self.pattern.rfind('/') {
            if last_slash == 0 {
                None
            } else {
                Some(self.pattern[..last_slash].to_string())
            }
        } else {
            None
        }
    }

    pub fn with_layout_option(mut self, option: LayoutOption) -> Self {
        self.layout_option = option;
        self
    }

    pub fn with_no_layout(self) -> Self {
        self.with_layout_option(LayoutOption::None)
    }

    pub fn with_root_layout(self) -> Self {
        self.with_layout_option(LayoutOption::Root)
    }

    pub fn with_named_layout(self, name: impl Into<String>) -> Self {
        self.with_layout_option(LayoutOption::Named(name.into()))
    }

    pub fn with_layout_pattern(self, pattern: impl Into<String>) -> Self {
        self.with_layout_option(LayoutOption::Pattern(pattern.into()))
    }

    pub fn with_meta(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata.extend(metadata);
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.contains_key(key)
    }

    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    pub fn with_aliases<I, S>(mut self, aliases: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.aliases.extend(aliases.into_iter().map(|s| s.into()));
        self
    }

    pub fn matches_any(&self, path: &str) -> Option<HashMap<String, String>> {
        if let Some(params) = self.matches(path) {
            return Some(params);
        }

        self.aliases.iter().find_map(|alias_pattern| {
            if self.matches_static_alias(path, alias_pattern) {
                Some(HashMap::new())
            } else {
                None
            }
        })
    }

    pub fn matches_static_alias(&self, path: &str, alias: &str) -> bool {
        let normalized_path = path.trim_end_matches('/');
        let normalized_alias = alias.trim_end_matches('/');

        if normalized_path.is_empty() && normalized_alias.is_empty() {
            return true;
        }

        normalized_path == normalized_alias
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn generate_url(&self, params: &HashMap<String, String>) -> Option<String> {
        let segments: Vec<&str> = self.pattern.split('/').filter(|s| !s.is_empty()).collect();

        let result_segments: Option<Vec<String>> = segments
            .iter()
            .map(|segment| match segment.chars().next() {
                Some(':') => {
                    let param_name = segment.trim_start_matches(':').trim_end_matches('?');

                    if segment.ends_with('?') {
                        Some(params.get(param_name).cloned().unwrap_or_default())
                    } else {
                        params.get(param_name).cloned()
                    }
                }
                Some('*') => {
                    let param_name = &segment[1..];
                    params.get(param_name).cloned()
                }
                _ => Some(segment.to_string()),
            })
            .collect(); // Collect into Option<Vec<String>>

        result_segments.map(|segs| {
            let filtered: Vec<String> = segs.into_iter().filter(|s| !s.is_empty()).collect();

            if filtered.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", filtered.join("/"))
            }
        })
    }

    pub fn redirect(
        from_pattern: impl Into<String>,
        to_url: impl Into<String>,
        status: u16,
    ) -> Self {
        let from = from_pattern.into();
        let target = to_url.into();

        let has_params = from.contains('[') || from.contains(':');

        let normalized_from = if from.contains(':') && !from.contains('[') {
            let mut result = String::new();
            let segments: Vec<&str> = from.split('/').collect();
            for (i, segment) in segments.iter().enumerate() {
                if i > 0 {
                    result.push('/');
                }
                if let Some(param) = segment.strip_prefix(':') {
                    result.push('[');
                    result.push_str(param);
                    result.push(']');
                } else {
                    result.push_str(segment);
                }
            }
            result
        } else {
            from.clone()
        };

        let (pattern, params, optional_params, dynamic_count, has_catch_all, param_constraints, has_optional_catch_all) =
            if has_params {
                routing::route::parse_pattern(&normalized_from)
            } else {
                let normalized = if from.starts_with('/') {
                    from.clone()
                } else {
                    format!("/{}", from)
                };
                (normalized, Vec::new(), Vec::new(), 0, false, HashMap::new(), false)
            };

        let depth = pattern.matches('/').count();
        let priority = routing::route::calculate_priority(
            has_catch_all,
            has_optional_catch_all,
            dynamic_count,
            depth,
            &optional_params,
        );

        Route {
            pattern,
            template_path: format!("redirect:{}", target),
            params,
            priority,
            is_layout: false,
            has_catch_all,
            optional_params,
            is_error_page: false,
            is_nolayout_marker: false,
            is_loading: false,
            is_template: false,
            is_not_found: false,
            is_parallel_route: false,
            parallel_slot: None,
            is_intercepting: false,
            intercept_level: None,
            intercept_target: None,
            layout_option: LayoutOption::None,
            layout_name: None,
            metadata: HashMap::new(),
            param_constraints,
            aliases: Vec::new(),
            name: None,
            is_redirect: true,
            redirect_to: Some(target),
            redirect_status: Some(status),
        }
    }

    pub fn redirect_target(&self, params: &HashMap<String, String>) -> Option<String> {
        if !self.is_redirect {
            return None;
        }

        let target = self.redirect_to.as_ref()?;

        if params.is_empty() {
            return Some(target.clone());
        }

        let mut result = target.clone();
        for (param_name, param_value) in params {
            let placeholder = format!(":{}", param_name);
            result = result.replace(&placeholder, param_value);
        }

        Some(result)
    }
}

#[derive(Clone)]
pub struct Router {
    routes: Vec<Route>,
    layouts: HashMap<String, Route>,
    named_layouts: HashMap<String, Route>,
    named_routes: HashMap<String, Route>,
    error_pages: HashMap<String, Route>,
    loading_pages: HashMap<String, Route>,
    templates: HashMap<String, Route>,
    not_found_pages: HashMap<String, Route>,
    parallel_routes: HashMap<String, HashMap<String, Route>>,
    intercepting_routes: HashMap<String, Route>,
    nolayout_patterns: std::collections::HashSet<String>,
    case_insensitive: bool,
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),
            layouts: HashMap::new(),
            named_layouts: HashMap::new(),
            named_routes: HashMap::new(),
            error_pages: HashMap::new(),
            loading_pages: HashMap::new(),
            templates: HashMap::new(),
            not_found_pages: HashMap::new(),
            parallel_routes: HashMap::new(),
            intercepting_routes: HashMap::new(),
            nolayout_patterns: std::collections::HashSet::new(),
            case_insensitive: false,
        }
    }

    pub fn with_case_insensitive(case_insensitive: bool) -> Self {
        Self {
            routes: Vec::new(),
            layouts: HashMap::new(),
            named_layouts: HashMap::new(),
            named_routes: HashMap::new(),
            error_pages: HashMap::new(),
            loading_pages: HashMap::new(),
            templates: HashMap::new(),
            not_found_pages: HashMap::new(),
            parallel_routes: HashMap::new(),
            intercepting_routes: HashMap::new(),
            nolayout_patterns: std::collections::HashSet::new(),
            case_insensitive,
        }
    }

    pub fn set_case_insensitive(&mut self, case_insensitive: bool) {
        self.case_insensitive = case_insensitive;
    }

    pub fn add_route(&mut self, route: Route) {
        if route.is_nolayout_marker {
            self.nolayout_patterns.insert(route.pattern.clone());
            return;
        }

        if let Some(ref name) = route.name {
            self.named_routes.insert(name.clone(), route.clone());
        }

        if route.is_layout {
            self.layouts.insert(route.pattern.clone(), route.clone());

            if let Some(ref name) = route.layout_name {
                self.named_layouts.insert(name.clone(), route);
            }
        } else if route.is_error_page {
            self.error_pages.insert(route.pattern.clone(), route);
        } else if route.is_loading {
            self.loading_pages.insert(route.pattern.clone(), route);
        } else if route.is_template {
            self.templates.insert(route.pattern.clone(), route);
        } else if route.is_not_found {
            self.not_found_pages.insert(route.pattern.clone(), route);
        } else if route.is_parallel_route {
            if let Some(ref slot) = route.parallel_slot {
                self.parallel_routes
                    .entry(route.pattern.clone())
                    .or_default()
                    .insert(slot.clone(), route);
            }
        } else if route.is_intercepting {
            self.intercepting_routes
                .insert(route.pattern.clone(), route);
        } else {
            self.routes.push(route);
            self.routes.sort_by_key(|r| r.priority);
        }
    }

    pub fn remove_route(&mut self, pattern: &str) {
        if let Some(pos) = self.routes.iter().position(|r| r.pattern == pattern) {
            let route = &self.routes[pos];
            if let Some(name) = &route.name {
                self.named_routes.remove(name);
            }
            self.routes.remove(pos);
        }

        if let Some(layout) = self.layouts.remove(pattern)
            && let Some(name) = &layout.layout_name
        {
            self.named_layouts.remove(name);
        }

        self.error_pages.remove(pattern);
        self.loading_pages.remove(pattern);
        self.templates.remove(pattern);
        self.not_found_pages.remove(pattern);
        self.parallel_routes.remove(pattern);
        self.intercepting_routes.remove(pattern);
    }

    pub fn sort_routes(&mut self) {
        self.routes.sort_by_key(|r| r.priority);
    }

    fn get_scoped_resource<'a>(
        &'a self,
        pattern: &str,
        map: &'a HashMap<String, Route>,
    ) -> Option<&'a Route> {
        let normalized = normalize_path(pattern);

        PathHierarchy::new(&normalized).find_map(|path| map.get(path))
    }

    pub fn match_route(&self, path: &str) -> Option<RouteMatch<'_>> {
        self.routes.iter().find_map(|route| {
            if let Some(params) = route.matches_with_options(path, self.case_insensitive) {
                return Some(RouteMatch { route, params });
            }

            route.aliases.iter().find_map(|alias| {
                if route.matches_static_alias(path, alias) {
                    Some(RouteMatch {
                        route,
                        params: HashMap::new(),
                    })
                } else {
                    None
                }
            })
        })
    }

    pub fn get_layout(&self, pattern: &str) -> Option<&Route> {
        self.get_scoped_resource(pattern, &self.layouts)
    }

    pub fn get_layout_for_match(&self, route_match: &RouteMatch<'_>) -> Option<&Route> {
        self.get_layout_with_option(&route_match.route.pattern, &route_match.route.layout_option)
    }

    pub fn get_layout_with_option(&self, pattern: &str, option: &LayoutOption) -> Option<&Route> {
        match option {
            LayoutOption::None => None,

            LayoutOption::Root => self.layouts.get("/"),

            LayoutOption::Named(name) => self.named_layouts.get(name),

            LayoutOption::Pattern(pat) => {
                let normalized = normalize_path(pat);
                self.layouts.get(normalized.as_ref())
            }

            LayoutOption::Inherit => {
                if self.is_under_nolayout_marker(pattern) {
                    return None;
                }
                self.get_scoped_resource(pattern, &self.layouts)
            }
        }
    }

    pub fn is_under_nolayout_marker(&self, pattern: &str) -> bool {
        let normalized = normalize_path(pattern);

        PathHierarchy::new(&normalized).any(|path| self.nolayout_patterns.contains(path))
    }

    pub fn get_layout_by_name(&self, name: &str) -> Option<&Route> {
        self.named_layouts.get(name)
    }

    pub fn routes(&self) -> &[Route] {
        &self.routes
    }

    pub fn layouts(&self) -> &HashMap<String, Route> {
        &self.layouts
    }

    pub fn get_error_page(&self, pattern: &str) -> Option<&Route> {
        self.get_scoped_resource(pattern, &self.error_pages)
    }

    pub fn error_pages(&self) -> &HashMap<String, Route> {
        &self.error_pages
    }

    pub fn get_loading_page(&self, pattern: &str) -> Option<&Route> {
        self.get_scoped_resource(pattern, &self.loading_pages)
    }

    pub fn loading_pages(&self) -> &HashMap<String, Route> {
        &self.loading_pages
    }

    pub fn get_template(&self, pattern: &str) -> Option<&Route> {
        self.get_scoped_resource(pattern, &self.templates)
    }

    pub fn templates(&self) -> &HashMap<String, Route> {
        &self.templates
    }

    pub fn get_not_found_page(&self, pattern: &str) -> Option<&Route> {
        self.get_scoped_resource(pattern, &self.not_found_pages)
    }

    pub fn not_found_pages(&self) -> &HashMap<String, Route> {
        &self.not_found_pages
    }

    pub fn get_parallel_routes(&self, pattern: &str) -> Option<&HashMap<String, Route>> {
        self.parallel_routes.get(pattern)
    }

    pub fn parallel_routes(&self) -> &HashMap<String, HashMap<String, Route>> {
        &self.parallel_routes
    }

    pub fn get_parallel_route(&self, pattern: &str, slot: &str) -> Option<&Route> {
        self.parallel_routes
            .get(pattern)
            .and_then(|slots| slots.get(slot))
    }

    pub fn get_intercepting_route(&self, pattern: &str) -> Option<&Route> {
        self.intercepting_routes.get(pattern)
    }

    pub fn intercepting_routes(&self) -> &HashMap<String, Route> {
        &self.intercepting_routes
    }

    pub fn url_for(&self, name: &str, params: &HashMap<String, String>) -> Option<String> {
        self.named_routes
            .get(name)
            .and_then(|route| route.generate_url(params))
    }

    pub fn url_for_params(&self, name: &str, params: &[(&str, &str)]) -> Option<String> {
        let param_map: HashMap<String, String> = params
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        self.url_for(name, &param_map)
    }

    pub fn get_route_by_name(&self, name: &str) -> Option<&Route> {
        self.named_routes.get(name)
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
