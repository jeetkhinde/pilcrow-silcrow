//! CSRF protection for form submissions.
//!
//! Applied as an always-on tower layer in the generated `build_router()`. Matches
//! SvelteKit's default: reject state-changing form submissions (POST/PUT/PATCH/DELETE
//! with a form-shaped Content-Type) whose `Origin` header doesn't match the request
//! `Host`. Cross-origin JSON APIs are left alone because the browser's same-origin
//! policy (plus CORS preflight) already protects them.
//!
//! Missing `Origin` falls back to `Referer` — rejected only when both are absent.

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, Method, Response, StatusCode, Uri},
    middleware::Next,
    response::IntoResponse,
};

const FORM_CONTENT_TYPES: &[&str] = &[
    "application/x-www-form-urlencoded",
    "multipart/form-data",
    "text/plain",
];

/// Tower middleware that enforces Origin-header CSRF protection on form submissions.
pub async fn csrf_middleware(req: Request, next: Next) -> Response<Body> {
    if !needs_csrf_check(req.method(), req.headers()) {
        return next.run(req).await;
    }

    let host = match request_host(req.headers()) {
        Some(h) => h,
        None => return reject("missing Host header"),
    };

    let origin_host = extract_origin_host(req.headers());
    let referer_host = extract_referer_host(req.headers());

    match origin_host.or(referer_host) {
        Some(ref h) if h.eq_ignore_ascii_case(&host) => next.run(req).await,
        Some(_) => reject("cross-origin form submission blocked"),
        None => reject("missing Origin and Referer on state-changing request"),
    }
}

fn needs_csrf_check(method: &Method, headers: &HeaderMap) -> bool {
    if !matches!(
        method,
        &Method::POST | &Method::PUT | &Method::PATCH | &Method::DELETE
    ) {
        return false;
    }
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    // Match by prefix — browsers may append `; boundary=…` or `; charset=…`.
    FORM_CONTENT_TYPES
        .iter()
        .any(|allowed| ct.trim_start().to_ascii_lowercase().starts_with(allowed))
}

fn request_host(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_ascii_lowercase())
}

fn extract_origin_host(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get("origin").and_then(|v| v.to_str().ok())?;
    // `Origin: null` is emitted by some sandboxed contexts — treat as missing.
    if raw == "null" {
        return None;
    }
    parse_host_from_url(raw)
}

fn extract_referer_host(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get("referer").and_then(|v| v.to_str().ok())?;
    parse_host_from_url(raw)
}

fn parse_host_from_url(url: &str) -> Option<String> {
    let uri: Uri = url.parse().ok()?;
    let host = uri.host()?.to_ascii_lowercase();
    match uri.port_u16() {
        Some(p) => Some(format!("{host}:{p}")),
        None => Some(host),
    }
}

fn reject(reason: &'static str) -> Response<Body> {
    (StatusCode::FORBIDDEN, format!("CSRF: {reason}")).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderName, HeaderValue, Method};

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                HeaderName::from_bytes(k.as_bytes()).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        h
    }

    #[test]
    fn safe_methods_never_trigger_check() {
        let h = headers(&[("content-type", "application/x-www-form-urlencoded")]);
        assert!(!needs_csrf_check(&Method::GET, &h));
        assert!(!needs_csrf_check(&Method::HEAD, &h));
        assert!(!needs_csrf_check(&Method::OPTIONS, &h));
    }

    #[test]
    fn post_form_content_type_triggers_check() {
        for ct in FORM_CONTENT_TYPES {
            let h = headers(&[("content-type", ct)]);
            assert!(needs_csrf_check(&Method::POST, &h), "{ct}");
        }
    }

    #[test]
    fn post_with_json_content_type_skipped() {
        let h = headers(&[("content-type", "application/json")]);
        assert!(!needs_csrf_check(&Method::POST, &h));
    }

    #[test]
    fn content_type_with_boundary_still_matches() {
        let h = headers(&[("content-type", "multipart/form-data; boundary=abc")]);
        assert!(needs_csrf_check(&Method::POST, &h));
    }

    #[test]
    fn extract_host_handles_port() {
        assert_eq!(
            parse_host_from_url("http://example.com:3000/path"),
            Some("example.com:3000".to_owned())
        );
        assert_eq!(
            parse_host_from_url("https://example.com/"),
            Some("example.com".to_owned())
        );
    }

    #[test]
    fn null_origin_is_ignored() {
        let h = headers(&[("origin", "null")]);
        assert_eq!(extract_origin_host(&h), None);
    }

    #[test]
    fn x_forwarded_host_beats_host() {
        let h = headers(&[
            ("host", "backend:3000"),
            ("x-forwarded-host", "app.example.com"),
        ]);
        assert_eq!(request_host(&h), Some("app.example.com".to_owned()));
    }
}
