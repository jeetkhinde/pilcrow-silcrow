use std::collections::HashMap;

/// Per-field options parsed from `#[revalidate(N)]` and `#[depends_on("key")]` on
/// `LiveProp<T>` fields in `Props`. Mutually exclusive: set one or the other, not both.
///
/// Stripped from emitted source — never reaches runtime code directly.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LiveFieldAttr {
    /// `#[revalidate(N)]` — auto-wires a `ScheduledInvalidation` for this field: every N seconds
    /// the dep key `"{module_name}::{field_name}"` is invalidated, triggering a re-bake.
    /// Also auto-injects that dep key into the field's `depends_on` in the generated
    /// `from_row()` impl when no explicit `depends_on` is set in `live.rs`.
    pub revalidate_secs: Option<u64>,
    /// `#[depends_on("some:key")]` — wires the field to a static dep key managed elsewhere.
    /// Mutually exclusive with `revalidate_secs`.
    pub depends_on: Option<String>,
}

/// SSG options parsed from `pub const` declarations in code-behind files.
///
/// All constants are stripped from the emitted module — they never reach runtime code.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SsgOpts {
    /// Whether the code-behind declares `pub async fn entries()`.
    /// Required for dynamic routes (patterns with `:param` segments) when `PROMOTE_AFTER = 0`.
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
    /// `Some(0)` means "promote on the very first hit". `None` defers to per-field or
    /// WatcherConfig default.
    pub promote_after: Option<u32>,
    /// Per-field options parsed from `#[revalidate(N)]` / `#[depends_on("key")]` on Props fields.
    /// Keyed by field name.
    pub live_field_attrs: HashMap<String, LiveFieldAttr>,
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
/// pub const FSR_JSON: bool = true;           // FSR: opt in to baked JSON alongside baked HTML
/// pub const PROMOTE_AFTER: u32 = 100;        // FSR: threshold; 0 = bake on first hit
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
