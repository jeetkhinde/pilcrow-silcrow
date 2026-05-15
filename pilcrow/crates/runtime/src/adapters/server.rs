use axum::Router;

use crate::adapter::{AdapterFuture, PilcrowAdapter, shutdown_signal};

/// Adapter for cloud platforms that inject a `PORT` env var at runtime.
///
/// Binds to `0.0.0.0:{PORT}` when the env var is present; falls back to the
/// configured `bind_addr` from `Pilcrow.toml` otherwise. This covers:
/// Fly.io, Railway, Render, Google Cloud Run, and Heroku.
///
/// # Usage
///
/// ```rust,ignore
/// pilcrow_web::start_with_adapter(pilcrow_router(), |_| async {}, FlyAdapter).await;
/// ```
pub struct PortEnvAdapter;

/// Fly.io adapter — reads `PORT` env var, binds on all interfaces.
pub type FlyAdapter = PortEnvAdapter;
/// Railway adapter — reads `PORT` env var, binds on all interfaces.
pub type RailwayAdapter = PortEnvAdapter;
/// Google Cloud Run adapter — reads `PORT` env var, binds on all interfaces.
pub type CloudRunAdapter = PortEnvAdapter;
/// Render adapter — reads `PORT` env var, binds on all interfaces.
pub type RenderAdapter = PortEnvAdapter;
/// Vercel long-running (non-Lambda) adapter — reads `PORT` env var.
pub type VercelAdapter = PortEnvAdapter;

impl PilcrowAdapter for PortEnvAdapter {
    fn serve(self, bind_addr: &str, app: Router) -> AdapterFuture {
        // let addr = match std::env::var("PORT") {
        //     Ok(port) => format!("0.0.0.0:{port}"),
        //     Err(_) => bind_addr.to_string(),
        // };
        let addr = resolve_addr(bind_addr);
        Box::pin(async move {
            let listener = match tokio::net::TcpListener::bind(&addr).await {
                Ok(listener) => listener,
                Err(err) => {
                    tracing::error!(addr = %addr, error = %err, "failed to bind server");
                    eprintln!("pilcrow: failed to bind {addr}: {err}");
                    std::process::exit(1);
                }
            };
            tracing::info!("listening on http://{addr}");
            if let Err(err) = axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await
            {
                tracing::error!(error = %err, "server failed");
                eprintln!("pilcrow: server failed: {err}");
                std::process::exit(1);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addr_uses_port_env_when_set() {
        assert_eq!(
            resolve_addr_impl("127.0.0.1:3000", Some("8080".into())),
            "0.0.0.0:8080"
        );
    }

    #[test]
    fn addr_falls_back_to_bind_addr_when_port_absent() {
        assert_eq!(resolve_addr_impl("127.0.0.1:3000", None), "127.0.0.1:3000");
    }

    #[test]
    fn addr_preserves_non_loopback_bind_addr_as_fallback() {
        assert_eq!(resolve_addr_impl("0.0.0.0:4000", None), "0.0.0.0:4000");
    }
}

fn resolve_addr(bind_addr: &str) -> String {
    resolve_addr_impl(bind_addr, std::env::var("PORT").ok())
}

fn resolve_addr_impl(bind_addr: &str, port: Option<String>) -> String {
    match port {
        Some(port) => format!("0.0.0.0:{port}"),
        None => bind_addr.to_string(),
    }
}
