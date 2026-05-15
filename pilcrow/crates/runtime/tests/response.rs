/// Unit tests for response builders and `ResponseExt` modifiers.
use axum::{http::StatusCode, response::IntoResponse};
use http_body_util::BodyExt;
use runtime::{
    AsyncHtmlPatch, AsyncValuePatch, ToastLevel, async_response_combined, async_value_response,
    response::response::{ResponseExt, form_errors, json, navigate, redirect},
};

// ── Helpers ──────────────────────────────────────────────────

async fn body_string(response: axum::response::Response) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn header_str<'a>(resp: &'a axum::response::Response, name: &str) -> Option<&'a str> {
    resp.headers().get(name)?.to_str().ok()
}

// ── redirect() ───────────────────────────────────────────────

#[test]
fn redirect_returns_ok() {
    assert!(redirect("/home").is_ok());
}

#[test]
fn redirect_status_is_303() {
    let resp = redirect("/home").unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
}

#[test]
fn redirect_sets_location_header() {
    let resp = redirect("/home").unwrap();
    assert_eq!(header_str(&resp, "location"), Some("/home"));
}

// ── navigate() ───────────────────────────────────────────────

#[test]
fn navigate_status_is_303() {
    let resp = navigate("/dashboard").into_response();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
}

#[test]
fn navigate_sets_location_header() {
    let resp = navigate("/dashboard").into_response();
    assert_eq!(header_str(&resp, "location"), Some("/dashboard"));
}

// ── json() ───────────────────────────────────────────────────

#[tokio::test]
async fn json_status_is_200() {
    let resp = json(serde_json::json!({"ok": true})).into_response();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn json_content_type_is_application_json() {
    let resp = json(serde_json::json!({"ok": true})).into_response();
    let ct = header_str(&resp, "content-type").unwrap_or("");
    assert!(ct.contains("application/json"), "content-type was: {ct}");
}

#[tokio::test]
async fn json_body_serializes_value() {
    let resp = json(serde_json::json!({"name": "alice"})).into_response();
    let body: serde_json::Value = serde_json::from_str(&body_string(resp).await).unwrap();
    assert_eq!(body["name"], "alice");
}

// ── form_errors() ────────────────────────────────────────────

#[test]
fn form_errors_error_sets_field_and_flag() {
    let fe = form_errors().error("email", "Required");
    assert!(fe.has_errors);
    assert_eq!(fe.errors.get("email").map(String::as_str), Some("Required"));
}

#[test]
fn form_errors_value_echoes_input() {
    let fe = form_errors().value("email", "bad@example.com");
    assert_eq!(
        fe.values.get("email").map(String::as_str),
        Some("bad@example.com")
    );
}

#[test]
fn form_errors_error_list_accumulates() {
    let fe = form_errors()
        .error("email", "Required")
        .error("password", "Too short");
    assert_eq!(fe.error_list.len(), 2);
    assert_eq!(fe.error_list[0].field, "email");
    assert_eq!(fe.error_list[1].field, "password");
}

#[test]
fn form_errors_is_valid_when_empty() {
    let fe = form_errors();
    assert!(fe.is_valid());
    assert!(!fe.has_errors);
}

#[test]
fn form_errors_into_response_is_json() {
    let resp = form_errors().error("x", "y").into_response();
    let ct = header_str(&resp, "content-type").unwrap_or("");
    assert!(ct.contains("application/json"), "content-type was: {ct}");
}

// ── ResponseExt ──────────────────────────────────────────────

#[test]
fn response_ext_with_header_adds_header() {
    let resp = navigate("/")
        .with_header("x-custom", "hello")
        .into_response();
    assert_eq!(header_str(&resp, "x-custom"), Some("hello"));
}

#[test]
fn response_ext_no_cache_sets_silcrow_cache_header() {
    let resp = navigate("/").no_cache().into_response();
    assert_eq!(header_str(&resp, "silcrow-cache"), Some("no-cache"));
}

#[tokio::test]
async fn async_value_response_requests_full_reload_for_boosted_navigation() {
    let resp = async_value_response("shell".to_owned(), futures_util::stream::empty());
    assert_eq!(header_str(&resp, "silcrow-full-reload"), Some("true"));
}

#[tokio::test]
async fn async_value_response_streams_direct_scalar_patch_script() {
    let resp = async_value_response(
        "shell".to_owned(),
        futures_util::stream::iter([AsyncValuePatch {
            field: "slow_count",
            json: "8".to_owned(),
        }]),
    );
    let body = body_string(resp).await;
    assert!(body.contains("window.__pilcrow_async_value(\"slow_count\",8)"));
    assert!(!body.contains("Silcrow.patch"));
}

#[tokio::test]
async fn async_response_combined_streams_direct_html_patch_script() {
    let resp = async_response_combined(
        "shell".to_owned(),
        futures_util::stream::empty(),
        futures_util::stream::iter([AsyncHtmlPatch {
            slot: "post_list",
            html: "<ul><li>Hi</li></ul>".to_owned(),
        }]),
    );
    let body = body_string(resp).await;
    assert!(body.contains("window.__pilcrow_async_html(\"post_list\""));
    assert!(body.contains("<ul><li>Hi</li></ul>"));
    assert!(!body.contains("window.__pd"));
}

#[test]
fn response_ext_with_status_overrides_status() {
    let resp = navigate("/")
        .with_status(StatusCode::TEMPORARY_REDIRECT)
        .into_response();
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
}

#[test]
fn response_ext_with_toast_sets_silcrow_toasts_cookie() {
    let resp = navigate("/")
        .with_toast("Saved!", ToastLevel::Success)
        .into_response();

    let cookies: Vec<_> = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap_or("").to_owned())
        .collect();

    let toast_cookie = cookies.iter().find(|c| c.contains("silcrow_toasts="));
    assert!(
        toast_cookie.is_some(),
        "expected silcrow_toasts cookie, got: {cookies:?}"
    );
}

#[test]
fn response_ext_trigger_event_sets_silcrow_trigger_header() {
    let resp = navigate("/").trigger_event("item:created").into_response();
    let trigger = header_str(&resp, "silcrow-trigger").unwrap_or("");
    assert!(
        trigger.contains("item:created"),
        "silcrow-trigger was: {trigger}"
    );
}

#[test]
fn response_ext_retarget_sets_silcrow_retarget_header() {
    let resp = navigate("/").retarget("#results").into_response();
    assert_eq!(header_str(&resp, "silcrow-retarget"), Some("#results"));
}

#[test]
fn response_ext_push_history_sets_silcrow_push_header() {
    let resp = navigate("/").push_history("/new-url").into_response();
    assert_eq!(header_str(&resp, "silcrow-push"), Some("/new-url"));
}

#[test]
fn response_ext_client_navigate_sets_silcrow_navigate_header() {
    let resp = navigate("/").client_navigate("/elsewhere").into_response();
    assert_eq!(header_str(&resp, "silcrow-navigate"), Some("/elsewhere"));
}

#[test]
fn response_ext_invalidate_target_sets_silcrow_invalidate_header() {
    let resp = navigate("/").invalidate_target("#list").into_response();
    // Single-call case: header is a JSON array with one selector.
    assert_eq!(header_str(&resp, "silcrow-invalidate"), Some("[\"#list\"]"));
}

#[test]
fn response_ext_invalidate_target_accumulates() {
    let resp = navigate("/")
        .invalidate_target("#list")
        .invalidate_target("#count")
        .into_response();
    let hdr = header_str(&resp, "silcrow-invalidate").unwrap_or("");
    let parsed: Vec<String> = serde_json::from_str(hdr).expect("parse invalidate header");
    assert_eq!(parsed, vec!["#list".to_owned(), "#count".to_owned()]);
}

#[test]
fn response_ext_patch_target_accumulates() {
    let resp = navigate("/")
        .patch_target("#cart", &serde_json::json!({"n": 1}))
        .patch_target("#badge", &serde_json::json!({"label": "new"}))
        .into_response();
    let hdr = header_str(&resp, "silcrow-patch").unwrap_or("");
    let parsed: Vec<serde_json::Value> = serde_json::from_str(hdr).expect("parse patch header");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0]["target"], "#cart");
    assert_eq!(parsed[0]["data"]["n"], 1);
    assert_eq!(parsed[1]["target"], "#badge");
    assert_eq!(parsed[1]["data"]["label"], "new");
}

#[test]
fn navigate_with_toast_and_header_both_applied() {
    let resp = navigate("/")
        .with_toast("Done", ToastLevel::Info)
        .with_header("x-op", "create")
        .into_response();

    assert_eq!(header_str(&resp, "x-op"), Some("create"));
    let cookies: Vec<_> = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap_or("").to_owned())
        .collect();
    assert!(cookies.iter().any(|c| c.contains("silcrow_toasts=")));
}
