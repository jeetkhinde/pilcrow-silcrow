use axum::Router;
use axum::body::Body as AxumBody;
use bytes::Bytes;
use http_body_util::BodyExt;
use lambda_http::Body as LambdaBody;
use tower::ServiceExt;

use crate::adapter::{AdapterFuture, PilcrowAdapter};

/// AWS Lambda adapter via the `lambda_http` crate.
///
/// Bridges `lambda_http::Request` / `lambda_http::Response` bodies to axum's
/// body types on each invocation. The `bind_addr` from `Pilcrow.toml` is
/// ignored — Lambda manages the network binding.
///
/// # Cargo.toml
///
/// ```toml
/// [dependencies]
/// pilcrow-web = { path = "…", features = ["lambda"] }
/// ```
///
/// # main.rs
///
/// ```rust,ignore
/// pilcrow_web::pilcrow_app!();
///
/// #[tokio::main]
/// async fn main() {
///     pilcrow_web::start_with_adapter(
///         pilcrow_router(),
///         |_| async {},
///         pilcrow_web::adapters::LambdaAdapter,
///     )
///     .await;
/// }
/// ```
///
/// Works with AWS Lambda, Vercel Functions (Lambda-backed), and Netlify Functions.
///
/// # Version note
///
/// Pinned to `lambda_http` 0.13 in `Cargo.toml`. `lambda_http` 1.x is available
/// (1.2.0 as of 2026); the `Body` enum (`Empty`, `Text`, `Binary`) and the
/// `run` / `service_fn` surface are unchanged, so upgrading should be mechanical,
/// but requires re-running the full lambda test suite before bumping the version.
pub struct LambdaAdapter;

impl PilcrowAdapter for LambdaAdapter {
    fn serve(
        self,
        _bind_addr: &str,
        app: Router,
        _on_bind: Box<dyn FnOnce(&str) + Send + 'static>,
    ) -> AdapterFuture {
        Box::pin(async move {
            if let Err(err) =
                lambda_http::run(lambda_http::service_fn(move |req: lambda_http::Request| {
                    let app = app.clone();
                    async move {
                        let (parts, lambda_body) = req.into_parts();
                        let body_bytes: Bytes = match lambda_body {
                            LambdaBody::Empty => Bytes::new(),
                            LambdaBody::Text(s) => Bytes::from(s.into_bytes()),
                            LambdaBody::Binary(b) => Bytes::from(b),
                        };
                        let http_req = http::Request::from_parts(parts, AxumBody::from(body_bytes));

                        let resp =
                            app.oneshot(http_req)
                                .await
                                .map_err(|e| -> lambda_http::Error {
                                    Box::new(std::io::Error::other(e.to_string()))
                                })?;

                        let (resp_parts, resp_body) = resp.into_parts();
                        let resp_bytes = resp_body
                            .collect()
                            .await
                            .map_err(|e| -> lambda_http::Error {
                                Box::new(std::io::Error::other(e.to_string()))
                            })?
                            .to_bytes();

                        Ok::<lambda_http::Response<LambdaBody>, lambda_http::Error>(
                            lambda_http::Response::from_parts(
                                resp_parts,
                                LambdaBody::from(resp_bytes.to_vec()),
                            ),
                        )
                    }
                }))
                .await
            {
                tracing::error!(error = %err, "lambda runtime failed");
                eprintln!("pilcrow: lambda runtime failed: {err}");
                std::process::exit(1);
            }
        })
    }
}
