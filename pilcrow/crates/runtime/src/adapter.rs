use std::future::Future;
use std::pin::Pin;

use axum::Router;

/// Pluggable deployment adapter.
///
/// Receives the fully-wired `Router` (ISR + middleware layers applied) and the
/// `bind_addr` from `Pilcrow.toml`, and is responsible for binding and serving.
///
/// The default [`TokioAdapter`] binds a `tokio::net::TcpListener` with graceful
/// shutdown on SIGTERM / Ctrl-C.
///
/// # Custom adapter example
///
/// ```rust,ignore
/// struct LambdaAdapter;
///
/// impl PilcrowAdapter for LambdaAdapter {
///     fn serve(self, _bind_addr: &str, app: Router, _on_bind: Box<dyn FnOnce(&str) + Send + 'static>) -> AdapterFuture {
///         Box::pin(async move {
///             lambda_http::run(app).await.expect("lambda serve");
///         })
///     }
/// }
///
/// // In main.rs:
/// pilcrow_web::start_with_adapter(pilcrow_router(), |_| async {}, LambdaAdapter).await;
/// ```
pub trait PilcrowAdapter: Send + 'static {
    fn serve(
        self,
        bind_addr: &str,
        app: Router,
        on_bind: Box<dyn FnOnce(&str) + Send + 'static>,
    ) -> AdapterFuture;
}

/// Boxed pinned future returned by [`PilcrowAdapter::serve`].
///
/// Does not require `Send` because adapters are always driven from `main()`,
/// not from a spawned task.
pub type AdapterFuture = Pin<Box<dyn Future<Output = ()>>>;

/// Default adapter: binds a `tokio::net::TcpListener` with graceful shutdown.
pub struct TokioAdapter;

impl PilcrowAdapter for TokioAdapter {
    fn serve(
        self,
        bind_addr: &str,
        app: Router,
        on_bind: Box<dyn FnOnce(&str) + Send + 'static>,
    ) -> AdapterFuture {
        let bind_addr = bind_addr.to_string();
        Box::pin(async move {
            let listener = 'bind: {
                // Try the configured address first, then scan up to 10 subsequent ports.
                let (host, port) = bind_addr
                    .rsplit_once(':')
                    .and_then(|(h, p)| p.parse::<u16>().ok().map(|p| (h, p)))
                    .unwrap_or(("127.0.0.1", 3000));
                let mut last_err = None;
                for offset in 0u16..=10 {
                    let candidate = format!("{host}:{}", port.saturating_add(offset));
                    match tokio::net::TcpListener::bind(&candidate).await {
                        Ok(listener) => {
                            if offset > 0 {
                                eprintln!(
                                    "pilcrow: port {} in use, using http://{candidate} instead",
                                    port
                                );
                            }
                            break 'bind listener;
                        }
                        Err(err) => last_err = Some((candidate, err)),
                    }
                }
                let (addr, err) = last_err.unwrap();
                tracing::error!(addr = %addr, error = %err, "failed to bind server");
                eprintln!("pilcrow: failed to bind any port starting at {port}: {err}");
                std::process::exit(1);
            };
            let bound = listener.local_addr().unwrap();
            tracing::info!("listening on http://{bound}");
            eprintln!("pilcrow: listening on http://{bound}");
            on_bind(&bound.to_string());
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

pub(super) async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %err, "failed to install Ctrl+C handler");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let sigterm = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = sigterm => {},
    }

    tracing::info!("shutdown signal received — draining in-flight requests");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct CallbackAdapter;

    impl PilcrowAdapter for CallbackAdapter {
        fn serve(
            self,
            _bind_addr: &str,
            _app: Router,
            on_bind: Box<dyn FnOnce(&str) + Send + 'static>,
        ) -> AdapterFuture {
            on_bind("127.0.0.1:5050");
            Box::pin(async {})
        }
    }

    #[tokio::test]
    async fn on_bind_callback_receives_resolved_addr() {
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let cap = captured.clone();
        let cb: Box<dyn FnOnce(&str) + Send + 'static> =
            Box::new(move |addr| *cap.lock().unwrap() = Some(addr.to_string()));

        CallbackAdapter
            .serve("127.0.0.1:3000", Router::new(), cb)
            .await;

        assert_eq!(
            *captured.lock().unwrap(),
            Some("127.0.0.1:5050".to_string())
        );
    }
}
