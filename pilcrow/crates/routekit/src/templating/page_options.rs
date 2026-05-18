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
    /// Route-level promotion threshold (`pub const PROMOTE_AFTER: u32 = N`).
    ///
    /// Overrides the per-field `#[pilcrow::promote_after]` attribute for the entire route.
    /// `Some(0)` means "promote on the very first hit" — equivalent to `PRERENDER = true`
    /// for FSR routes. `None` means defer to the per-field or WatcherConfig default.
    pub promote_after: Option<u32>,
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
/// pub const PRERENDER: bool = true;          // SSG: pre-render at server startup (FSR: sets promote_after = 0)
/// pub const FSR_JSON: bool = true;           // FSR: opt in to baked JSON alongside baked HTML
/// pub const PROMOTE_AFTER: u32 = 100;        // FSR: override per-field promote_after for the whole route
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageOptions {
    pub trailing_slash: TrailingSlash,
    pub layout: LayoutOpt,
    pub ssg: SsgOpts,
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
