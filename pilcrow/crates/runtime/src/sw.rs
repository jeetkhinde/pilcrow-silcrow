use std::sync::Arc;

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use pilcrow_core::config::config::{ServiceWorkerConfig, SwStrategy};

use crate::assets::assets::SILCROW_JS;

// Injected before </body> in every text/html response when SW is enabled.
const SW_REGISTRATION: &str = concat!(
    "<script>(function(){",
    "if('serviceWorker'in navigator){",
    "navigator.serviceWorker.register('/sw.js').catch(function(){});",
    "}})()</script>",
);

/// Generates the full `/sw.js` source from the current `ServiceWorkerConfig`.
///
/// The cache name embeds a CRC32 of the strategy, precache list, and silcrow.js
/// content — so it auto-busts on any configuration or silcrow.js change.
pub fn generate_sw_source(config: &ServiceWorkerConfig) -> String {
    let mut h = crc32fast::Hasher::new();
    h.update(SILCROW_JS.as_bytes());
    h.update(format!("{:?}", config.strategy).as_bytes());
    for url in &config.precache {
        h.update(url.as_bytes());
    }
    let version = h.finalize();
    let cache_name = format!("pilcrow-{version:08x}");

    // silcrow.js has a content-hashed URL — safe to precache forever.
    let silcrow_path = crate::assets::assets::silcrow_js_path();
    let mut precache_urls: Vec<String> = vec![silcrow_path];
    precache_urls.extend(config.precache.clone());
    let precache_json = serde_json::to_string(&precache_urls).unwrap_or_else(|_| "[]".into());

    let mut excludes: Vec<String> = vec!["/__pilcrow/".into()];
    excludes.extend(config.exclude.clone());
    let excludes_json = serde_json::to_string(&excludes).unwrap_or_else(|_| "[]".into());

    let fetch_handler = fetch_handler_js(&config.strategy, config.offline_fallback.as_deref());

    let mut out = String::with_capacity(2048);
    out.push_str("'use strict';\n");
    out.push_str(&format!("var CACHE='{cache_name}';\n"));
    out.push_str(&format!("var PRECACHE={precache_json};\n"));
    out.push_str(&format!("var EXCL={excludes_json};\n"));
    out.push_str("self.addEventListener('install',function(e){");
    out.push_str("self.skipWaiting();");
    out.push_str("e.waitUntil(caches.open(CACHE).then(function(c){return c.addAll(PRECACHE);}));");
    out.push_str("});\n");
    out.push_str("self.addEventListener('activate',function(e){");
    out.push_str("e.waitUntil(caches.keys().then(function(ks){");
    out.push_str("return Promise.all(ks.filter(function(k){return k.indexOf('pilcrow-')===0&&k!==CACHE;}).map(function(k){return caches.delete(k);}));");
    out.push_str("}).then(function(){return self.clients.claim();}));");
    out.push_str("});\n");
    out.push_str("function skip(u){");
    out.push_str("if(u.indexOf('chrome-extension')!==-1)return true;");
    out.push_str("for(var i=0;i<EXCL.length;i++){if(u.indexOf(EXCL[i])!==-1)return true;}");
    out.push_str("return false;");
    out.push_str("}\n");
    out.push_str(&fetch_handler);
    out.push('\n');
    out.push_str("self.addEventListener('fetch',function(e){");
    out.push_str("if(e.request.method!=='GET')return;");
    out.push_str("if(skip(e.request.url))return;");
    out.push_str("e.respondWith(handle(e.request));");
    out.push_str("});\n");
    out
}

fn fetch_handler_js(strategy: &SwStrategy, offline_fallback: Option<&str>) -> String {
    let fallback = match offline_fallback {
        Some(path) => format!("caches.match({path:?})"),
        None => "Promise.resolve(new Response('',{status:503}))".into(),
    };

    match strategy {
        SwStrategy::NetworkFirst => format!(
            "function cacheable(r){{var cc=r.headers.get('cache-control')||'';return r.ok&&cc.indexOf('no-store')===-1&&cc.indexOf('private')===-1;}}\
            function handle(req){{\
              return fetch(req).then(function(r){{\
                if(cacheable(r))caches.open(CACHE).then(function(c){{c.put(req,r.clone());}});\
                return r;\
              }}).catch(function(){{\
                return caches.match(req).then(function(c){{return c||{fallback};}});\
              }});\
            }}"
        ),
        SwStrategy::CacheFirst => format!(
            "function cacheable(r){{var cc=r.headers.get('cache-control')||'';return r.ok&&cc.indexOf('no-store')===-1&&cc.indexOf('private')===-1;}}\
            function handle(req){{\
              return caches.match(req).then(function(c){{\
                if(c)return c;\
                return fetch(req).then(function(r){{\
                  if(cacheable(r))caches.open(CACHE).then(function(ca){{ca.put(req,r.clone());}});\
                  return r;\
                }}).catch(function(){{return {fallback};}});\
              }});\
            }}"
        ),
        SwStrategy::StaleWhileRevalidate => format!(
            "function cacheable(r){{var cc=r.headers.get('cache-control')||'';return r.ok&&cc.indexOf('no-store')===-1&&cc.indexOf('private')===-1;}}\
            function handle(req){{\
              return caches.open(CACHE).then(function(cache){{\
                return cache.match(req).then(function(cached){{\
                  var fresh=fetch(req).then(function(r){{\
                    if(cacheable(r))cache.put(req,r.clone());\
                    return r;\
                  }}).catch(function(){{return {fallback};}});\
                  return cached||fresh;\
                }});\
              }});\
            }}"
        ),
    }
}

/// Axum handler for `GET /sw.js`. Generates the service worker source on each request.
///
/// The response is served with `Cache-Control: no-store` so the browser always
/// re-fetches to detect SW updates.
pub async fn sw_handler(
    axum::Extension(config): axum::Extension<Arc<pilcrow_core::PilcrowConfig>>,
) -> Response {
    let source = generate_sw_source(&config.service_worker);
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        source,
    )
        .into_response()
}

/// Axum middleware that injects the SW registration snippet before `</body>` in
/// every `text/html` response. Only added to the router when SW is enabled.
pub async fn sw_inject_layer(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let response = next.run(request).await;

    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("text/html"))
        .unwrap_or(false);

    if !is_html {
        return response;
    }

    const SW_BODY_LIMIT_BYTES: usize = 10 * 1024 * 1024;
    let (mut parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, SW_BODY_LIMIT_BYTES).await {
        Ok(b) => b,
        Err(_) => return axum::response::Response::from_parts(parts, axum::body::Body::empty()),
    };

    let mut html = String::from_utf8_lossy(&bytes).into_owned();
    if let Some(pos) = html.rfind("</body>") {
        html.insert_str(pos, SW_REGISTRATION);
    } else {
        html.push_str(SW_REGISTRATION);
    }

    parts.headers.remove(header::CONTENT_LENGTH);
    axum::response::Response::from_parts(parts, axum::body::Body::from(html))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pilcrow_core::config::config::ServiceWorkerConfig;

    #[test]
    fn sw_excludes_pilcrow_prefix_not_silcrow() {
        let config = ServiceWorkerConfig::default();
        let source = generate_sw_source(&config);
        assert!(
            source.contains("/__pilcrow/"),
            "should exclude /__pilcrow/ prefix: {source}"
        );
        assert!(
            !source.contains("/_silcrow/"),
            "/_silcrow/ exclude should be removed (now under /__pilcrow/): {source}"
        );
    }

    #[test]
    fn sw_precaches_silcrow_at_new_path() {
        let config = ServiceWorkerConfig::default();
        let source = generate_sw_source(&config);
        assert!(
            source.contains("/__pilcrow/runtime/silcrow."),
            "silcrow.js should be precached at /__pilcrow/runtime/: {source}"
        );
    }
}
