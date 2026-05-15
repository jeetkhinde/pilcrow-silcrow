use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use fluent_bundle::FluentArgs;
use fluent_bundle::FluentResource;
use fluent_bundle::FluentValue;
use fluent_bundle::concurrent::FluentBundle;
use unic_langid::LanguageIdentifier;

// ── Bundle storage ────────────────────────────────────────────

type ConcurrentBundle = FluentBundle<Arc<FluentResource>>;

struct BundleStore {
    bundles: HashMap<String, ConcurrentBundle>,
    default_locale: String,
    all_locales: Vec<String>,
}

/// Pre-loaded Fluent translation bundles, one per locale.
///
/// Loaded once at startup and shared across all requests via `Arc`.
/// Clone is cheap — it clones the `Arc` pointer only.
#[derive(Clone)]
pub struct I18nBundles(Arc<BundleStore>);

impl std::fmt::Debug for I18nBundles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("I18nBundles")
            .field("locales", &self.0.all_locales)
            .field("default", &self.0.default_locale)
            .finish()
    }
}

impl I18nBundles {
    /// Load all configured locales from `locales_dir/{locale}/*.ftl`.
    ///
    /// Missing locale directories produce a warning but don't fail startup.
    pub fn load(locales_dir: &Path, locales: &[String], default_locale: &str) -> Self {
        let mut bundles = HashMap::with_capacity(locales.len());

        for locale in locales {
            let locale_dir = locales_dir.join(locale);
            match load_bundle(&locale_dir, locale) {
                Ok(bundle) => {
                    bundles.insert(locale.clone(), bundle);
                }
                Err(e) => {
                    tracing::warn!("i18n: failed to load locale '{locale}': {e}");
                }
            }
        }

        Self(Arc::new(BundleStore {
            bundles,
            default_locale: default_locale.to_string(),
            all_locales: locales.to_vec(),
        }))
    }

    /// Translate a message key using `locale`, falling back to the default locale.
    ///
    /// If the key is missing in both, the key is returned verbatim.
    pub fn translate(&self, locale: &str, key: &str, args: &[(&str, &str)]) -> String {
        let store = &*self.0;

        // Split "button.label" → ("button", Some("label"))
        let (msg_id, attr_name) = if let Some(dot) = key.find('.') {
            (&key[..dot], Some(&key[dot + 1..]))
        } else {
            (key, None)
        };

        let try_locale = |loc: &str| -> Option<String> {
            let bundle = store.bundles.get(loc)?;
            let msg = bundle.get_message(msg_id)?;

            let pattern = if let Some(attr) = attr_name {
                msg.attributes().find(|a| a.id() == attr)?.value()
            } else {
                msg.value()?
            };

            let mut fluent_args = FluentArgs::new();
            for (k, v) in args {
                fluent_args.set(*k, FluentValue::from(*v));
            }
            let mut errors = Vec::new();
            Some(
                bundle
                    .format_pattern(pattern, Some(&fluent_args), &mut errors)
                    .into_owned(),
            )
        };

        // Try requested locale first, then default locale, then return the key.
        try_locale(locale)
            .or_else(|| try_locale(&store.default_locale.clone()))
            .unwrap_or_else(|| key.to_string())
    }

    pub fn default_locale(&self) -> &str {
        &self.0.default_locale
    }

    pub fn all_locales(&self) -> &[String] {
        &self.0.all_locales
    }
}

fn load_bundle(locale_dir: &Path, locale: &str) -> std::io::Result<ConcurrentBundle> {
    let lang_id: LanguageIdentifier = locale
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{e}")))?;

    let mut bundle = ConcurrentBundle::new_concurrent(vec![lang_id]);

    let entries = match std::fs::read_dir(locale_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(
                "i18n: locale directory '{}' not found — no messages loaded for '{locale}'",
                locale_dir.display()
            );
            return Ok(bundle);
        }
        Err(e) => return Err(e),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ftl") {
            continue;
        }
        let source = std::fs::read_to_string(&path)?;
        match FluentResource::try_new(source) {
            Ok(res) => {
                if let Err(errors) = bundle.add_resource(Arc::new(res)) {
                    for e in errors {
                        tracing::warn!("i18n: {}: {e}", path.display());
                    }
                }
            }
            Err((res, errors)) => {
                for e in &errors {
                    tracing::warn!("i18n: FTL parse error in {}: {e}", path.display());
                }
                // Add the partially-parsed resource so valid messages still work.
                let _ = bundle.add_resource(Arc::new(res));
            }
        }
    }

    Ok(bundle)
}

// ── Locale detection (extension marker) ──────────────────────

/// Locale code stored in request extensions by the locale-rewrite middleware.
#[derive(Clone, Debug, Default)]
pub struct CurrentLocale(pub String);

// ── Locale-rewrite middleware ─────────────────────────────────

/// Detects a non-default locale prefix from the URL path, strips it, and stores
/// the detected locale in request extensions so `Req.locale` is populated.
///
/// Strategy: **prefix-except-default**.
/// - Default locale (e.g. `en`) → served at `/products` (no prefix).
/// - Other locales → `/de/products`, `/fr/products`, etc.
///
/// Called from `start.rs` with a captured `I18nBundles` clone when `[i18n]` is configured.
pub async fn locale_middleware_impl(
    mut req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
    bundles: I18nBundles,
) -> axum::response::Response {
    let path = req.uri().path().to_string();
    let store = &*bundles.0;

    let mut detected = store.default_locale.clone();

    'outer: for locale in &store.all_locales {
        if locale == &store.default_locale {
            continue;
        }
        let prefix = format!("/{locale}");
        if path == prefix.as_str() || path.starts_with(&format!("{prefix}/")) {
            detected = locale.clone();
            let stripped = &path[prefix.len()..];
            let new_path = if stripped.is_empty() { "/" } else { stripped };

            // Preserve query string when rewriting the URI.
            let new_pq = match req.uri().query() {
                Some(q) => format!("{new_path}?{q}"),
                None => new_path.to_string(),
            };

            if let Ok(pq) = new_pq.parse::<axum::http::uri::PathAndQuery>() {
                let mut parts = req.uri().clone().into_parts();
                parts.path_and_query = Some(pq);
                if let Ok(new_uri) = axum::http::Uri::from_parts(parts) {
                    *req.uri_mut() = new_uri;
                }
            }
            break 'outer;
        }
    }

    req.extensions_mut().insert(CurrentLocale(detected));
    next.run(req).await
}

// ── Formatting helper ─────────────────────────────────────────

/// Locale-aware formatting helper, obtained via `req.fmt()`.
///
/// The backend uses simple heuristics based on locale code. CLDR-accurate
/// formatting can be plugged in later by swapping the implementation here.
pub struct FmtHelper<'a> {
    pub locale: &'a str,
}

impl<'a> FmtHelper<'a> {
    /// Format an integer with locale-appropriate thousands separators.
    ///
    /// ```rust,ignore
    /// req.fmt().number(1_234_567)  // "1,234,567" (en) | "1.234.567" (de)
    /// ```
    pub fn number(&self, n: i64) -> String {
        format_integer(n, thousands_sep(self.locale))
    }

    /// Format a float with locale-appropriate separators.
    ///
    /// ```rust,ignore
    /// req.fmt().float(1234.5, 2)  // "1,234.50" (en) | "1.234,50" (de)
    /// ```
    pub fn float(&self, n: f64, decimals: usize) -> String {
        let sep = thousands_sep(self.locale);
        let dec = decimal_sep(self.locale);
        let abs = n.abs();
        let int_part = abs.floor() as i64;
        let frac = abs - abs.floor();

        let int_str = format_integer(int_part, sep);
        let sign = if n < 0.0 { "-" } else { "" };

        if decimals == 0 {
            return format!("{sign}{int_str}");
        }

        let frac_str = format!("{:.decimals$}", frac);
        let frac_digits = &frac_str[2..]; // strip "0."
        format!("{sign}{int_str}{dec}{frac_digits}")
    }
}

fn thousands_sep(locale: &str) -> char {
    let lang = locale.split('-').next().unwrap_or(locale);
    match lang {
        "de" | "nl" | "pl" | "ru" | "tr" | "cs" | "sk" | "hr" => '.',
        "fr" | "es" | "it" | "pt" | "sv" | "fi" | "no" | "nb" | "da" => '\u{202F}', // narrow no-break space
        _ => ',',
    }
}

fn decimal_sep(locale: &str) -> char {
    let lang = locale.split('-').next().unwrap_or(locale);
    match lang {
        "de" | "nl" | "pl" | "ru" | "tr" | "cs" | "sk" | "hr" | "fr" | "es" | "it" | "pt"
        | "sv" | "fi" | "no" | "nb" | "da" => ',',
        _ => '.',
    }
}

fn format_integer(n: i64, sep: char) -> String {
    let abs_str = n.unsigned_abs().to_string();
    let mut result = String::with_capacity(abs_str.len() + abs_str.len() / 3 + 1);

    for (i, ch) in abs_str.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(sep);
        }
        result.push(ch);
    }

    if n < 0 {
        result.push('-');
    }

    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_integer_en() {
        let h = FmtHelper { locale: "en" };
        assert_eq!(h.number(1_234_567), "1,234,567");
        assert_eq!(h.number(-42), "-42");
        assert_eq!(h.number(0), "0");
    }

    #[test]
    fn format_integer_de() {
        let h = FmtHelper { locale: "de" };
        assert_eq!(h.number(1_234_567), "1.234.567");
    }

    #[test]
    fn format_float_en() {
        let h = FmtHelper { locale: "en" };
        assert_eq!(h.float(1234.5, 2), "1,234.50");
    }

    #[test]
    fn format_float_de() {
        let h = FmtHelper { locale: "de" };
        assert_eq!(h.float(1234.5, 2), "1.234,50");
    }
}
