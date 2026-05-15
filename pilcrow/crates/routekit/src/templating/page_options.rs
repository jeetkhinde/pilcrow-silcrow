/// ISR options parsed from `pub const` declarations in code-behind files.
///
/// All constants are stripped from the emitted module — they never reach runtime code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IsrOpts {
    /// Cache TTL in seconds (`REVALIDATE`). `None` means ISR is disabled for this page.
    pub revalidate: Option<u64>,
    /// Maximum stale age in seconds before falling back to blocking render (`MAX_STALE`).
    pub max_stale: Option<u64>,
    /// Tag names for group invalidation (`CACHE_TAGS`).
    pub cache_tags: Vec<String>,
    /// Locals keys whose values scope the cache key (`CACHE_VARY`).
    pub cache_vary: Vec<String>,
}

impl IsrOpts {
    /// `true` when this page has ISR enabled (i.e. `REVALIDATE` is set).
    pub fn is_active(&self) -> bool {
        self.revalidate.is_some()
    }
}

/// SSG options parsed from `pub const` declarations in code-behind files.
///
/// All constants are stripped from the emitted module — they never reach runtime code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SsgOpts {
    /// Whether to pre-render this route at server startup (`PRERENDER`).
    pub prerender: bool,
    /// Whether the code-behind declares `pub async fn entries()`.
    /// Required for dynamic routes (patterns with `:param` segments).
    pub has_entries_fn: bool,
}

/// FSR options derived from filesystem discovery and `pub const` declarations.
///
/// All constants are stripped from the emitted module — they never reach runtime code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FsrOpts {
    /// `live.rs` was found alongside this page's `page.rs`.
    pub has_live_file: bool,
    /// `pub const FSR_JSON: bool = true` was declared in `page.rs`.
    pub json: bool,
}

impl FsrOpts {
    /// `true` when this page participates in FSR (has a `live.rs` companion).
    pub fn is_active(&self) -> bool {
        self.has_live_file
    }
}

/// Per-page options parsed from `pub const` declarations in code-behind files.
///
/// ```rust,ignore
/// pub const TRAILING_SLASH: &str = "always"; // "always" | "never" | "ignore"
/// pub const LAYOUT: &str = "none";           // opt out of all layout wrapping
/// pub const REVALIDATE: u64 = 60;            // ISR: cache TTL in seconds
/// pub const PRERENDER: bool = true;          // SSG: pre-render at server startup
/// pub const STREAMING: bool = true;          // SSR Streaming: shell renders immediately, page data streamed
/// pub const FSR_JSON: bool = true;           // FSR: opt in to baked JSON alongside baked HTML
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageOptions {
    pub trailing_slash: TrailingSlash,
    pub layout: LayoutOpt,
    pub isr: IsrOpts,
    pub ssg: SsgOpts,
    /// Whether the page uses SSR Streaming: layout loads run immediately, page `load()` is
    /// spawned in the background, and the shell renders before data arrives. The resolved
    /// `Props` are streamed as a single `Silcrow.patch()` call once `load()` completes.
    pub streaming: bool,
    /// FSR options — populated when a `live.rs` companion file is present.
    pub fsr: FsrOpts,
}

/// Whether this page participates in the automatic layout chain.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LayoutOpt {
    /// Default — inherit the full auto-layout chain from ancestor `_layout.html` files.
    #[default]
    Inherit,
    /// Strip all layout wrapping: the page renders its own template directly.
    None,
}

/// How the framework handles a trailing slash for this page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TrailingSlash {
    /// Default — no extra routes generated; axum matches the pattern as-is.
    #[default]
    Never,
    /// Redirect `GET /path` → `/path/` when a request arrives without a trailing slash.
    Always,
    /// Redirect `GET /path/` → `/path` when a request arrives with a trailing slash.
    Ignore,
}

impl TrailingSlash {
    pub fn from_label(s: &str) -> Self {
        match s.trim_matches('"').trim_matches('\'') {
            "always" => Self::Always,
            "ignore" => Self::Ignore,
            _ => Self::Never,
        }
    }
}
