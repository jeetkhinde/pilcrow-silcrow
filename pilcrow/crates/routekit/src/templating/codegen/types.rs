use super::*;

/// One generated API route entry for build-time manifests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedApiRoute {
    pub pattern: String,
    pub module_path: String,
    pub symbol: String,
}

/// Input for code-generating one compiled Askama template module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateCodegenInput {
    pub module_name: String,
    pub render_symbol: String,
    pub source_path: String,
    pub rust_frontmatter: String,
    pub template_source: String,
    /// Ordered layout chain for this page: [outermost_auto_layout, ..., explicit_layout].
    /// Each entry is a layout module name.  Only meaningful for `page_*` modules.
    pub layout_chain: Vec<String>,
    /// URL prefix for fragment directory entries (e.g. `"widgets"`). `None` for non-fragments.
    pub fragment_url_prefix: Option<String>,
    /// Route params generated from the file path for page modules.
    pub route_params: Vec<GeneratedRouteParam>,
}

/// Metadata for one generated template module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedTemplateEntry {
    pub source_path: String,
    pub module_name: String,
    pub render_symbol: String,
}

/// Metadata about the layout chain's contribution to a page module.
/// Keyed by page module name in `layout_fields_map`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutFieldsInfo {
    /// Ordered from outermost to innermost layout, one entry per layout in the chain
    /// that declares `load()`.  Each entry is `(layout_module_name, field_names)`.
    pub chain: Vec<(String, Vec<String>)>,
    /// Field names belonging to the page's own `Props` (for constructing `__MergedProps`).
    pub page_field_names: Vec<String>,
}

/// Shape of a page's `load()` function. Absent means the page is static.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadSignature {
    /// Whether `load` was declared `async fn`.
    pub is_async: bool,
    /// Whether the return type is `Result<_, _>` / `AppResult<_>`.
    pub returns_result: bool,
    /// Whether the parameter list declares a `PilcrowClient` argument.
    pub wants_client: bool,
    /// Whether the parameter list declares a `Req` argument.
    pub wants_req: bool,
    /// Whether the parameter list declares a generated `Page` context argument.
    pub wants_page: bool,
    /// Whether the parameter list declares a `Live` extractor argument.
    pub wants_live: bool,
}

impl LoadSignature {
    pub fn consumes_req(self) -> bool {
        self.wants_req || self.wants_page
    }
}

/// One dynamic parameter generated from a file-route segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedRouteParam {
    pub name: String,
    pub rust_type: String,
    pub optional: bool,
    pub catch_all: bool,
}

/// One discovered named action handler in a page's code-behind.
///
/// Named actions are `pub` fns whose signature matches
/// `pub async fn NAME(req: Req) -> ActionResult` (name other than `load`).
/// The client invokes them by POSTing to the page URL with `?/<name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionFn {
    /// Function name — also the URL key (`?/<name>`).
    pub name: String,
    pub is_async: bool,
    /// Whether the return type is `Result<_, _>` / `ActionResult`.
    pub returns_result: bool,
    /// Whether the parameter list declares a `Req` argument.
    pub wants_req: bool,
}

/// In-memory generated module + metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedTemplatesModule {
    pub source: String,
    pub entries: Vec<GeneratedTemplateEntry>,
    /// Map from module_name to `Some(sig)` when the frontmatter contains a `load()` fn.
    pub load_map: HashMap<String, Option<LoadSignature>>,
    /// Map from page module_name to layout field info, for pages whose layout has `load()`.
    pub layout_fields_map: HashMap<String, LayoutFieldsInfo>,
    /// Map from page module_name to its list of discovered named action handlers.
    pub action_map: HashMap<String, Vec<ActionFn>>,
    /// Map from page module_name to its per-page options (`TRAILING_SLASH`, etc.).
    pub page_options: HashMap<String, PageOptions>,
    /// Map from page module_name to its `Deferred<T>` (JSON patch) field names.
    pub deferred_fields_map: HashMap<String, Vec<String>>,
    /// Map from page module_name to its `DeferredHtml` (HTML slot) field names.
    pub deferred_html_fields_map: HashMap<String, Vec<String>>,
    /// Map from page module_name to its ISR configuration (only for pages with `REVALIDATE`).
    pub isr_config_map: HashMap<String, IsrOpts>,
    /// Map from page module_name to its SSG configuration (only for pages with `PRERENDER = true`).
    pub ssg_config_map: HashMap<String, SsgOpts>,
    /// Map from page module_name to its `LiveProp<T>` field names.
    pub live_fields_map: HashMap<String, Vec<String>>,
    /// Map from page module_name to whether a `live()` fn and/or `LiveProp` struct are present.
    pub has_live_fn_map: HashMap<String, bool>,
    /// Map from page module_name to processed live.rs source (with from_row() injected).
    pub fsr_live_source_map: HashMap<String, String>,
    /// Map from page module_name to LiveProp field names from live.rs.
    pub fsr_live_fields_map: HashMap<String, Vec<String>>,
}

/// Which server hook functions are present in `src/hooks.rs`.
///
/// Detected at build time by scanning the file for known `pub async fn` signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HookFlags {
    /// `pub async fn handle(req: Req, next: Next) -> Response` — request/response wrapping.
    pub has_handle: bool,
    /// `pub async fn handle_error(error: &HookError, req: &Req) -> Option<Response>` — 5xx interception.
    pub has_handle_error: bool,
    /// `pub async fn init()` — server startup initialisation.
    pub has_init: bool,
}

/// Result of instrumenting a template's Rust frontmatter.
pub struct InstrumentedFrontmatter {
    pub source: String,
    pub load_signature: Option<LoadSignature>,
    /// Named fields declared in the module's own `Props` struct (before any injection).
    pub own_syn_fields: Vec<syn::Field>,
    /// Named action handlers discovered in the frontmatter (pages only).
    pub actions: Vec<ActionFn>,
    /// Per-page options parsed from `pub const` declarations and stripped from output.
    /// `page_options.ssg.has_entries_fn` is set when `pub async fn entries()` is detected.
    pub page_options: PageOptions,
    /// Names of `Deferred<T>` (JSON patch) fields in `Props`, in declaration order.
    pub deferred_fields: Vec<String>,
    /// Names of `DeferredHtml` (HTML slot) fields in `Props`, in declaration order.
    pub deferred_html_fields: Vec<String>,
    /// Names of `LiveProp<T>` fields in `Props`, in declaration order.
    pub live_fields: Vec<String>,
    /// True when a `live()` fn or `LiveProp` struct is present in the frontmatter.
    pub has_live_fn: bool,
    /// Processed source from a sibling `live.rs` file (stripped + from_row injected), if any.
    pub fsr_live_source: Option<String>,
    /// Names of `LiveProp<T>` fields in `Live` struct from `live.rs`.
    pub fsr_live_fields: Vec<String>,
}
