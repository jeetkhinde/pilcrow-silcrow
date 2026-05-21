use std::env;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PilcrowConfig {
    #[serde(default)]
    pub web: WebConfig,
    #[serde(default)]
    pub backend: BackendConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub service_worker: ServiceWorkerConfig,
    #[serde(default)]
    pub i18n: I18nConfig,
    #[serde(default)]
    pub images: ImageConfig,
    #[serde(default)]
    pub client: ClientRuntimeConfig,
    #[serde(default)]
    pub live: LiveConfig,
    #[serde(default)]
    pub fsr: FsrConfig,
}

/// Configuration for the `[live]` section in `Pilcrow.toml`.
/// Controls live-props caching behaviour.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LiveConfig {
    /// Number of hits before a live route is promoted to baked HTML.
    pub promote_after_hits: u32,
    /// Seconds to debounce SSE patch events before sending to clients.
    pub patch_debounce_seconds: u32,
    /// Seconds after last hit before a pilcrow_cache row is eligible for purge.
    /// Defaults to 30 days.
    pub purge_after_seconds: u32,
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            promote_after_hits: 100,
            patch_debounce_seconds: 30,
            purge_after_seconds: 2_592_000,
        }
    }
}

/// Configuration for the `[fsr]` section in `Pilcrow.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FsrConfig {
    /// `"embedded"` (Tokio task inside Pilcrow) or `"external"` (caller-driven).
    pub watcher: String,
    /// How often the embedded watcher polls for stale rows (milliseconds).
    pub poll_interval_ms: u64,
    /// Framework default promote_after_hits.
    pub promote_after_hits: u32,
    /// Framework default patch_debounce_secs.
    pub patch_debounce_secs: u32,
    /// Seconds before stale baked artefacts are purged.
    pub purge_after_seconds: u64,
    /// Maximum concurrent SSE connections before returning 503.
    pub max_sse_connections: u32,
    /// Seconds before forcing a client reconnect (EventSource auto-reconnects).
    pub connection_ttl_secs: u64,
    /// SSE keep-alive heartbeat interval in seconds.
    pub keepalive_secs: u64,
    /// Redis connection URL (`redis://...`). When set, the embedded watcher uses
    /// Redis pub/sub instead of polling and the FSR cache layer is activated.
    pub redis_url: Option<String>,
}

impl Default for FsrConfig {
    fn default() -> Self {
        Self {
            watcher: "embedded".to_string(),
            poll_interval_ms: 500,
            promote_after_hits: 100,
            patch_debounce_secs: 30,
            purge_after_seconds: 2_592_000,
            max_sse_connections: 1000,
            connection_ttl_secs: 3600,
            keepalive_secs: 30,
            redis_url: None,
        }
    }
}

/// Runtime client-side feature configuration (mirrors the build-time `[client]` table).
///
/// ```toml
/// [client]
/// inline_runtime = true  # embed Silcrow JS inline instead of <script src>
/// ```
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ClientRuntimeConfig {
    #[serde(default)]
    pub react: ReactRuntimeConfig,
    /// Embed the Silcrow runtime inline in each HTML response rather than serving it
    /// from `/__pilcrow/runtime/silcrow.{hash}.js`. Eliminates the extra HTTP request
    /// at the cost of a larger initial HTML payload. Default: `false`.
    #[serde(default)]
    pub inline_runtime: bool,
}

/// Runtime React island configuration.
///
/// ```toml
/// [client.react]
/// ssr      = true        # allow strategy="ssr" and strategy="shell" at runtime
/// node_bin = "node"      # path to Node binary if not on PATH
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ReactRuntimeConfig {
    /// Allow `strategy="ssr"` islands — requires a persistent Node process at runtime.
    #[serde(default)]
    pub ssr: bool,
    /// Path to the Node.js binary. Defaults to `"node"`.
    #[serde(default = "default_node_bin")]
    pub node_bin: String,
}

impl Default for ReactRuntimeConfig {
    fn default() -> Self {
        Self {
            ssr: false,
            node_bin: default_node_bin(),
        }
    }
}

fn default_node_bin() -> String {
    "node".to_string()
}

/// Image optimisation configuration. Disabled by default (`enabled = false`).
///
/// ```toml
/// [images]
/// enabled     = true
/// cache_dir   = ".pilcrow-image-cache"
/// domains     = []              # allowed remote domains; empty = local-only
/// max_width   = 3840
/// max_height  = 2160
/// quality     = 75
/// formats     = ["webp", "jpeg"]   # preference order for format=auto
/// concurrency = 4              # max simultaneous transforms
/// ```
///
/// Use `<pilcrow:image src="..." width=N alt="..." />` in templates.
/// The framework serves optimised images at `GET /_image?src=...&w=...&f=...`.
#[derive(Debug, Clone, Deserialize)]
pub struct ImageConfig {
    /// Whether to serve the `/_image` optimisation endpoint. Default: `false`.
    #[serde(default)]
    pub enabled: bool,
    /// Disk directory for cached optimised images. Default: `".pilcrow-image-cache"`.
    #[serde(default = "default_image_cache_dir")]
    pub cache_dir: String,
    /// Remote hostnames allowed as `src` values (e.g. `["images.cdn.com"]`).
    /// Empty list = local images only (safer default).
    #[serde(default)]
    pub domains: Vec<String>,
    /// Maximum allowed output width in pixels. Default: 3840.
    #[serde(default = "default_max_image_dimension")]
    pub max_width: u32,
    /// Maximum allowed output height in pixels. Default: 2160.
    #[serde(default = "default_max_image_dimension")]
    pub max_height: u32,
    /// Default JPEG/WebP quality (1–100). Default: 75.
    #[serde(default = "default_image_quality")]
    pub quality: u8,
    /// Output format preference order when `f=auto`. Default: `["webp", "jpeg"]`.
    #[serde(default = "default_image_formats")]
    pub formats: Vec<String>,
    /// Maximum concurrent image transforms (CPU-bound). Default: 4.
    #[serde(default = "default_image_concurrency")]
    pub concurrency: usize,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cache_dir: default_image_cache_dir(),
            domains: Vec::new(),
            max_width: default_max_image_dimension(),
            max_height: default_max_image_dimension(),
            quality: default_image_quality(),
            formats: default_image_formats(),
            concurrency: default_image_concurrency(),
        }
    }
}

/// Internationalisation configuration. Opt-in: leave `locales` empty to disable i18n entirely.
///
/// ```toml
/// [i18n]
/// default_locale = "en"
/// locales        = ["en", "de", "fr"]
/// locales_dir    = "locales"   # relative to project root; default is "locales"
/// ```
///
/// - The default locale is served at bare URLs (`/products`).
/// - All other locales are served with a URL prefix (`/de/products`).
/// - `.ftl` files are loaded from `{locales_dir}/{locale}/*.ftl` at startup.
#[derive(Debug, Clone, Deserialize)]
pub struct I18nConfig {
    /// The locale served at bare URLs (no prefix). Default: `"en"`.
    #[serde(default = "default_locale_str")]
    pub default_locale: String,
    /// All supported locale codes. When empty, i18n is disabled.
    #[serde(default)]
    pub locales: Vec<String>,
    /// Directory containing per-locale `.ftl` files, relative to the project root.
    #[serde(default = "default_locales_dir")]
    pub locales_dir: String,
}

impl Default for I18nConfig {
    fn default() -> Self {
        Self {
            default_locale: default_locale_str(),
            locales: Vec::new(),
            locales_dir: default_locales_dir(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ServiceWorkerConfig {
    /// Whether to register and serve `/sw.js`. Disabled by default.
    #[serde(default)]
    pub enabled: bool,
    /// Caching strategy applied to all non-excluded GET requests.
    #[serde(default)]
    pub strategy: SwStrategy,
    /// Extra URLs to precache on service worker install (silcrow.js is always included).
    #[serde(default)]
    pub precache: Vec<String>,
    /// URL substrings to exclude from service worker interception.
    /// `/__pilcrow/` is always excluded (covers all Pilcrow runtime assets).
    #[serde(default)]
    pub exclude: Vec<String>,
    /// URL to serve when a request fails and no cached response exists.
    pub offline_fallback: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SwStrategy {
    /// Try network first; fall back to cache on failure. (default)
    #[default]
    NetworkFirst,
    /// Serve from cache immediately; fetch in background only on cache miss.
    CacheFirst,
    /// Serve cached response immediately while revalidating in background.
    StaleWhileRevalidate,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CacheConfig {
    #[serde(default)]
    pub provider: CacheProvider,
    /// Redis connection URL (`redis://...`). Required when `provider = "redis"`.
    pub url: Option<String>,
    /// SQLite database path. Used when `provider = "sqlite"`.
    pub path: Option<String>,
    /// Directory for filesystem-backed ISR cache. Used when `provider = "filesystem"`.
    /// Defaults to `.pilcrow-cache` in the current directory.
    pub dir: Option<String>,
    /// Maximum duration (seconds) a background revalidation task may run before abort.
    #[serde(default = "default_revalidate_timeout_secs")]
    pub revalidate_timeout_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            provider: CacheProvider::default(),
            url: None,
            path: None,
            dir: None,
            revalidate_timeout_secs: default_revalidate_timeout_secs(),
        }
    }
}

/// Which backing store to use for the ISR cache.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CacheProvider {
    /// In-process HashMap — zero config, no persistence across restarts.
    #[default]
    Memory,
    /// Filesystem JSON files — single-node persistence that survives restarts.
    /// Set `dir = ".pilcrow-cache"` to configure the storage directory.
    Filesystem,
    /// SQLite file — single-node persistence.
    Sqlite,
    /// Redis — multi-node shared cache.
    Redis,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_web_host")]
    pub host: String,
    #[serde(default = "default_web_port")]
    pub port: u16,
    #[serde(default = "default_backend_url")]
    pub backend_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendConfig {
    #[serde(default = "default_backend_host")]
    pub host: String,
    #[serde(default = "default_backend_port")]
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            host: default_web_host(),
            port: default_web_port(),
            backend_url: default_backend_url(),
        }
    }
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            host: default_backend_host(),
            port: default_backend_port(),
        }
    }
}

impl PilcrowConfig {
    pub fn load_from(start_dir: impl AsRef<Path>) -> io::Result<Self> {
        let start_dir = start_dir.as_ref();
        let mut config = match find_config_path(start_dir) {
            Some(path) => {
                let raw = std::fs::read_to_string(&path)?;
                toml::from_str::<Self>(&raw).map_err(|err| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("failed to parse {}: {err}", path.display()),
                    )
                })?
            }
            None => Self::default(),
        };
        config.apply_env_overrides()?;
        Ok(config)
    }

    pub fn load_from_current_dir() -> io::Result<Self> {
        let cwd = env::current_dir()?;
        Self::load_from(cwd)
    }

    pub fn web_bind_addr(&self) -> String {
        format!("{}:{}", self.web.host, self.web.port)
    }

    pub fn backend_bind_addr(&self) -> String {
        format!("{}:{}", self.backend.host, self.backend.port)
    }

    fn apply_env_overrides(&mut self) -> io::Result<()> {
        if let Some(host) = get_env("PILCROW_WEB_HOST")? {
            self.web.host = host;
        }
        if let Some(port) = get_env_u16("PILCROW_WEB_PORT")? {
            self.web.port = port;
        }
        if let Some(url) = get_env("PILCROW_BACKEND_URL")? {
            self.web.backend_url = url;
        }
        if let Some(host) = get_env("PILCROW_BACKEND_HOST")? {
            self.backend.host = host;
        }
        if let Some(port) = get_env_u16("PILCROW_BACKEND_PORT")? {
            self.backend.port = port;
        }
        Ok(())
    }
}

fn find_config_path(start_dir: &Path) -> Option<PathBuf> {
    let mut current = Some(start_dir);
    while let Some(dir) = current {
        let candidate = dir.join("Pilcrow.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

fn get_env(key: &str) -> io::Result<Option<String>> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("environment variable {key} is not valid unicode"),
        )),
    }
}

fn get_env_u16(key: &str) -> io::Result<Option<u16>> {
    match get_env(key)? {
        Some(raw) => raw.parse::<u16>().map(Some).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("environment variable {key} must be a valid port: {err}"),
            )
        }),
        None => Ok(None),
    }
}

fn default_locale_str() -> String {
    "en".to_string()
}

fn default_locales_dir() -> String {
    "locales".to_string()
}

fn default_revalidate_timeout_secs() -> u64 {
    30
}

fn default_web_host() -> String {
    "127.0.0.1".to_string()
}

fn default_web_port() -> u16 {
    3000
}

fn default_backend_url() -> String {
    "http://127.0.0.1:4000".to_string()
}

fn default_backend_host() -> String {
    "127.0.0.1".to_string()
}

fn default_backend_port() -> u16 {
    4000
}

fn default_image_cache_dir() -> String {
    ".pilcrow-image-cache".to_string()
}

fn default_max_image_dimension() -> u32 {
    3840
}

fn default_image_quality() -> u8 {
    75
}

fn default_image_formats() -> Vec<String> {
    vec!["webp".to_string(), "jpeg".to_string()]
}

fn default_image_concurrency() -> usize {
    4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fsr_config_new_fields_have_correct_defaults() {
        let cfg = FsrConfig::default();
        assert_eq!(cfg.max_sse_connections, 1000);
        assert_eq!(cfg.connection_ttl_secs, 3600);
        assert_eq!(cfg.keepalive_secs, 30);
    }

    #[test]
    fn fsr_config_new_fields_deserialize_from_toml() {
        let toml = r#"
            max_sse_connections = 500
            connection_ttl_secs  = 7200
            keepalive_secs       = 45
        "#;
        let cfg: FsrConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.max_sse_connections, 500);
        assert_eq!(cfg.connection_ttl_secs, 7200);
        assert_eq!(cfg.keepalive_secs, 45);
    }
}
