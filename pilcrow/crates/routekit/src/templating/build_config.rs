use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Build-time configuration read from `Pilcrow.toml` in the crate root.
/// Only fields relevant to the build pipeline are included here.
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct PilcrowBuildConfig {
    /// URL-accessible fragment groups (Option A: flat array, dir name → URL prefix).
    #[serde(default)]
    pub fragments: Vec<FragmentEntry>,

    /// Frontmatter import aliases. `ui = "ui"` is always available by default.
    #[serde(default = "default_import_aliases")]
    pub imports: HashMap<String, String>,

    /// Environment variable declarations — generates typed `env::Public` / `env::Private` structs.
    #[serde(default)]
    pub env: EnvConfig,

    /// Routing configuration (e.g. directories to ignore).
    #[serde(default)]
    pub routing: RoutingConfig,

    /// i18n configuration — generates typed `t::` translation functions from `.ftl` files.
    #[serde(default)]
    pub i18n: I18nBuildConfig,

    /// Client-side integrations, currently React client islands.
    #[serde(default)]
    pub client: ClientBuildConfig,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct ClientBuildConfig {
    #[serde(default)]
    pub react: ReactBuildConfig,
    #[serde(default)]
    pub solid: SolidBuildConfig,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ReactBuildConfig {
    /// Enables the React island build when `<react>` tags are present.
    #[serde(default)]
    pub enabled: bool,
    /// Directory names that may contain React source. These directories must also
    /// be excluded with `[routing].ignore_directories`.
    #[serde(default = "default_react_dirs")]
    pub dirs: Vec<String>,
    /// Warn when a single generated island entry exceeds this approximate size.
    #[serde(default)]
    pub max_island_kb: Option<u64>,
    /// Warn when all React JS for one build exceeds this approximate size.
    #[serde(default)]
    pub max_page_react_kb: Option<u64>,
    #[serde(default)]
    pub warn: bool,
    #[serde(default)]
    pub fail_on_budget: bool,
    /// Enable SSR strategies (`strategy="shell"` and `strategy="ssr"`).
    /// Requires Node.js at build time (shell) and optionally at runtime (ssr).
    #[serde(default)]
    pub ssr: bool,
    /// Path to the Node.js binary used for SSR rendering. Defaults to `"node"`.
    #[serde(default = "default_node_bin")]
    pub node_bin: String,
}

impl Default for ReactBuildConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dirs: default_react_dirs(),
            max_island_kb: None,
            max_page_react_kb: None,
            warn: false,
            fail_on_budget: false,
            ssr: false,
            node_bin: default_node_bin(),
        }
    }
}

fn default_node_bin() -> String {
    "node".to_string()
}

fn default_react_dirs() -> Vec<String> {
    vec!["react".to_string()]
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SolidBuildConfig {
    /// Enables the Solid island build when `<solid>` tags are present.
    #[serde(default)]
    pub enabled: bool,
    /// Directory names that may contain Solid source. These directories must also
    /// be excluded with `[routing].ignore_directories`.
    #[serde(default = "default_solid_dirs")]
    pub dirs: Vec<String>,
    /// Warn when a single generated island entry exceeds this approximate size.
    #[serde(default)]
    pub max_island_kb: Option<u64>,
    /// Warn when all Solid JS for one build exceeds this approximate size.
    #[serde(default)]
    pub max_page_solid_kb: Option<u64>,
    #[serde(default)]
    pub warn: bool,
    #[serde(default)]
    pub fail_on_budget: bool,
}

impl Default for SolidBuildConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dirs: default_solid_dirs(),
            max_island_kb: None,
            max_page_solid_kb: None,
            warn: false,
            fail_on_budget: false,
        }
    }
}

fn default_solid_dirs() -> Vec<String> {
    vec!["solid".to_string()]
}

/// Build-time i18n configuration. Mirrors `I18nConfig` in `pilcrow-core`.
///
/// The build pipeline reads FTL files from `{project_root}/{locales_dir}/{default_locale}/*.ftl`
/// and generates a `pub mod t { ... }` with one typed function per message key.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct I18nBuildConfig {
    /// Default locale code (e.g. `"en"`). FTL files from this locale are used to derive
    /// the generated `t::` function signatures.
    #[serde(default = "default_locale_str")]
    pub default_locale: String,
    /// All supported locale codes. When empty, an empty `pub mod t {}` is emitted.
    #[serde(default)]
    pub locales: Vec<String>,
    /// Directory containing per-locale `.ftl` files, relative to the project root.
    #[serde(default = "default_locales_dir")]
    pub locales_dir: String,
}

impl Default for I18nBuildConfig {
    fn default() -> Self {
        Self {
            default_locale: default_locale_str(),
            locales: Vec::new(),
            locales_dir: default_locales_dir(),
        }
    }
}

fn default_locale_str() -> String {
    "en".to_string()
}

fn default_locales_dir() -> String {
    "locales".to_string()
}

/// Routing configuration.
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct RoutingConfig {
    /// Directory names to ignore during route discovery.
    #[serde(default)]
    pub ignore_directories: Vec<String>,
}

/// Typed environment variable declarations.
///
/// ```toml
/// [env]
/// public  = ["PUBLIC_API_URL", "PUBLIC_APP_NAME"]
/// private = ["DATABASE_URL", "SECRET_KEY"]
/// ```
///
/// The build pipeline generates `pub mod env` with `Public` and `Private` structs, each
/// having typed `String` fields and a `load() -> Result<Self, std::env::VarError>` constructor.
/// Public var field names strip the `PUBLIC_` prefix; private keep the full snake-cased name.
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct EnvConfig {
    /// Vars intended for client-visible use.  `PUBLIC_API_URL` → field `api_url`.
    #[serde(default)]
    pub public: Vec<String>,
    /// Server-only vars.  `DATABASE_URL` → field `database_url`.
    #[serde(default)]
    pub private: Vec<String>,
}

impl EnvConfig {
    /// Convert an env var name to a Rust field name.
    /// Public vars have their `PUBLIC_` prefix stripped before conversion.
    pub fn field_name(var: &str, is_public: bool) -> String {
        let raw = if is_public {
            var.strip_prefix("PUBLIC_").unwrap_or(var)
        } else {
            var
        };
        raw.to_ascii_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect()
    }
}

/// One fragment directory group.
///
/// ```toml
/// [[fragments]]
/// dir = "widgets"               # required; relative to project root
///
/// [[fragments]]
/// dir = "ui-blocks"
/// url = "blocks"                # optional; overrides the URL prefix
/// ```
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FragmentEntry {
    /// Directory containing fragment HTML files, relative to the project root.
    /// e.g. `"widgets"` → `widgets/**/*.html`
    pub dir: String,

    /// URL prefix for routes generated from this directory.
    /// Defaults to the last path segment of `dir` (e.g. `"widgets"`).
    pub url: Option<String>,
}

impl FragmentEntry {
    /// Resolved URL prefix (no leading slash).
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

    /// Absolute path of the fragment directory given the crate manifest root.
    pub fn abs_dir(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.dir)
    }
}

fn default_import_aliases() -> HashMap<String, String> {
    HashMap::from([("ui".to_string(), "ui".to_string())])
}

impl PilcrowBuildConfig {
    /// Try to load from `Pilcrow.toml` in `manifest_dir`; silently returns default on missing/parse error.
    pub fn load_from(manifest_dir: &Path) -> Self {
        let path = manifest_dir.join("Pilcrow.toml");
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&content).unwrap_or_default()
    }
}
