pub mod types;
pub use self::types::*;
pub mod api_routes;
pub use self::api_routes::*;
pub mod page_routes;
pub use self::page_routes::*;
pub mod templates;
pub use self::templates::*;
pub mod instrument;
pub use self::instrument::*;
pub mod app_module;
pub use self::app_module::*;
pub mod emit;
pub use self::emit::*;
pub mod util;
pub use self::util::*;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use proc_macro2::Span;
use quote::ToTokens;
use syn::parse_quote;

use crate::routing::constraint::ParameterConstraint;
use crate::routing::discovery::{
    build_api_routes, build_fragment_routes, build_page_routes_with_fragment_dirs,
};
<<<<<<< HEAD
use crate::templating::page_options::{LayoutOpt, PageOptions, SsgOpts, TrailingSlash};
=======
use crate::templating::page_options::{LayoutOpt, PageOptions, TrailingSlash};
>>>>>>> origin/main

/// One generated page route entry for build-time manifests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedPageRoute {
    pub pattern: String,
    pub template_path: String,
    pub symbol: String,
    pub render_symbol: String,
    /// Typed route params generated from the file path, in route order.
    pub route_params: Vec<GeneratedRouteParam>,
    /// Param names mapped to their external matcher module names.
    /// e.g. `[id=integer]` → `{ "id" => "integer" }` (calls `crate::params::integer::match_param`).
    pub param_matchers: HashMap<String, String>,
}
fn build_fragment_symbol(template_path: &str, dir_norm: &str, url_prefix: &str) -> String {
    let path = normalize_path_text(Path::new(template_path));
    let relative = path
        .strip_prefix(dir_norm)
        .unwrap_or(&path)
        .trim_start_matches('/');
    let without_ext = relative.strip_suffix(".html").unwrap_or(relative);
    let prefixed = format!("{url_prefix}/{without_ext}");

    let mut symbol = String::new();
    let mut prev = false;
    for ch in prefixed.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if mapped == '_' {
            if !prev {
                symbol.push('_');
            }
            prev = true;
        } else {
            symbol.push(mapped);
            prev = false;
        }
    }
    let s = symbol.trim_matches('_');
    let s = if s.is_empty() { "index" } else { s };
    format!("frag_{s}")
}
/// Output from writing generated templates module.
pub struct WrittenTemplatesOutput {
    pub entries: Vec<GeneratedTemplateEntry>,
    pub load_map: HashMap<String, Option<LoadSignature>>,
    /// Forwarded from `GeneratedTemplatesModule`; passed on to the app module writer.
    pub layout_fields_map: HashMap<String, LayoutFieldsInfo>,
    pub action_map: HashMap<String, Vec<ActionFn>>,
    pub page_options: HashMap<String, PageOptions>,
<<<<<<< HEAD
    pub ssg_config_map: HashMap<String, SsgOpts>,
=======
>>>>>>> origin/main
    pub live_fields_map: HashMap<String, Vec<String>>,
    pub has_live_fn_map: HashMap<String, bool>,
    /// Map from page module_name to processed live.rs source (with from_row() injected).
    pub fsr_live_source_map: HashMap<String, String>,
    /// Map from page module_name to LiveProp field names from live.rs.
    pub fsr_live_fields_map: HashMap<String, Vec<String>>,
}
