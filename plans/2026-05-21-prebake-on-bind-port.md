# Prebake on_bind Port Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an `on_bind` callback to `PilcrowAdapter::serve()` so each adapter reports its actual bound address to `prebake::set_local_base`, instead of `start_with_adapter` guessing from `config.web.port`.

**Architecture:** Change the `PilcrowAdapter` trait to accept a `Box<dyn FnOnce(&str) + Send + 'static>` callback. Each adapter calls it once, right after binding the `TcpListener`, passing the real `local_addr()` string. `start_with_adapter` removes its premature `set_local_base` call and instead passes the closure. `LambdaAdapter` accepts but ignores the callback (no local HTTP server exists on Lambda).

**Tech Stack:** Rust, Tokio, Axum, `pilcrow-runtime` crate.

---

## Files

| File | Change |
|------|--------|
| `pilcrow/crates/runtime/src/adapter.rs` | Trait signature + `TokioAdapter` impl |
| `pilcrow/crates/runtime/src/adapters/server.rs` | `PortEnvAdapter` impl |
| `pilcrow/crates/runtime/src/adapters/lambda.rs` | `LambdaAdapter` impl (accept, ignore) |
| `pilcrow/crates/runtime/src/start.rs` | Remove old `set_local_base` call; pass `on_bind` closure |

---

## Task 1 — Write a failing test for the new trait signature

**Files:**
- Modify: `pilcrow/crates/runtime/src/adapter.rs`

- [ ] **Step 1: Add the failing test to `adapter.rs`**

Append this module at the bottom of `pilcrow/crates/runtime/src/adapter.rs`:

```rust
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
```

- [ ] **Step 2: Confirm it fails to compile**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime --lib 2>&1 | grep -E "error|warning: unused"
```

Expected: compilation error — `serve` has wrong number of arguments (the old trait only takes 2 params after `self`).

---

## Task 2 — Update the `PilcrowAdapter` trait and `TokioAdapter`

**Files:**
- Modify: `pilcrow/crates/runtime/src/adapter.rs`

- [ ] **Step 1: Update the trait signature**

Replace in `adapter.rs`:
```rust
pub trait PilcrowAdapter: Send + 'static {
    fn serve(self, bind_addr: &str, app: Router) -> AdapterFuture;
}
```
With:
```rust
pub trait PilcrowAdapter: Send + 'static {
    fn serve(
        self,
        bind_addr: &str,
        app: Router,
        on_bind: Box<dyn FnOnce(&str) + Send + 'static>,
    ) -> AdapterFuture;
}
```

- [ ] **Step 2: Update the doc-comment example in the trait**

The existing doc example in `adapter.rs` shows:
```rust
/// impl PilcrowAdapter for LambdaAdapter {
///     fn serve(self, _bind_addr: &str, app: Router) -> AdapterFuture {
```

Replace the example with:
```rust
/// impl PilcrowAdapter for LambdaAdapter {
///     fn serve(self, _bind_addr: &str, app: Router, _on_bind: Box<dyn FnOnce(&str) + Send + 'static>) -> AdapterFuture {
```

- [ ] **Step 3: Update `TokioAdapter::serve` to accept and call `on_bind`**

Replace the entire `TokioAdapter` impl in `adapter.rs`:

```rust
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
```

The only change vs. the original is: `on_bind` parameter added, and `on_bind(&bound.to_string());` called after the `eprintln!` but before `axum::serve`.

---

## Task 3 — Update `PortEnvAdapter`

**Files:**
- Modify: `pilcrow/crates/runtime/src/adapters/server.rs`

- [ ] **Step 1: Update `PortEnvAdapter::serve` to accept and call `on_bind`**

Replace the `impl PilcrowAdapter for PortEnvAdapter` block:

```rust
impl PilcrowAdapter for PortEnvAdapter {
    fn serve(
        self,
        bind_addr: &str,
        app: Router,
        on_bind: Box<dyn FnOnce(&str) + Send + 'static>,
    ) -> AdapterFuture {
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
            let bound = listener.local_addr().unwrap();
            tracing::info!("listening on http://{bound}");
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
```

Changes vs. original: `on_bind` parameter; capture `let bound = listener.local_addr().unwrap()`; call `on_bind(&bound.to_string())` before `axum::serve`; updated `tracing::info!` to use `bound` instead of `addr`.

---

## Task 4 — Update `LambdaAdapter`

**Files:**
- Modify: `pilcrow/crates/runtime/src/adapters/lambda.rs`

Lambda manages all networking; there is no local port to report. Accept the parameter and drop it without calling it — `prebake::trigger` will remain a no-op for Lambda deployments.

- [ ] **Step 1: Add `on_bind` parameter to `LambdaAdapter::serve`**

Replace:
```rust
impl PilcrowAdapter for LambdaAdapter {
    fn serve(self, _bind_addr: &str, app: Router) -> AdapterFuture {
```
With:
```rust
impl PilcrowAdapter for LambdaAdapter {
    fn serve(
        self,
        _bind_addr: &str,
        app: Router,
        _on_bind: Box<dyn FnOnce(&str) + Send + 'static>,
    ) -> AdapterFuture {
```

No other changes to `lambda.rs`.

---

## Task 5 — Update `start_with_adapter`

**Files:**
- Modify: `pilcrow/crates/runtime/src/start.rs`

- [ ] **Step 1: Remove the premature `set_local_base` call**

In `start.rs`, delete these two lines (around line 68–69):
```rust
    // Register the local base URL for prebake_next() / prebake::trigger().
    crate::prebake::set_local_base(format!("http://127.0.0.1:{}", config.web.port));
```

- [ ] **Step 2: Pass `on_bind` closure to `adapter.serve()`**

The last line of `start_with_adapter` is:
```rust
    adapter.serve(&bind_addr, app).await;
```

Replace it with:
```rust
    adapter.serve(&bind_addr, app, Box::new(|actual| {
        // Normalize 0.0.0.0 (all-interfaces bind) to loopback for local prebake requests.
        let base = if let Some(port) = actual.strip_prefix("0.0.0.0:") {
            format!("http://127.0.0.1:{port}")
        } else {
            format!("http://{actual}")
        };
        crate::prebake::set_local_base(base);
    })).await;
```

---

## Task 6 — Run tests and commit

- [ ] **Step 1: Run all runtime tests**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime --lib 2>&1 | tail -15
```

Expected: `test result: ok. N passed; 0 failed`

- [ ] **Step 2: Run with `live-props` feature**

```bash
cargo test --manifest-path pilcrow/Cargo.toml -p pilcrow-runtime --lib --features live-props 2>&1 | tail -5
```

Expected: `test result: ok. N passed; 0 failed`

- [ ] **Step 3: Run MCP tests**

```bash
cargo test --manifest-path pilcrow/tools/pilcrow-mcp/Cargo.toml 2>&1 | tail -5
```

Expected: `test result: ok. N passed; 0 failed`

- [ ] **Step 4: Update `registry.toml` — `windowed-prebake` spec text**

In `pilcrow/registry.toml`, find the `[[features]]` entry with `id = "windowed-prebake"` and update the `spec` field. Replace:

```toml
The local base URL is set once at startup by start_with_adapter:
  prebake::set_local_base(format!("http://127.0.0.1:{}", config.web.port))
```

With:

```toml
The local base URL is set once at startup by start_with_adapter via the
on_bind callback passed to PilcrowAdapter::serve(). Each adapter calls
on_bind with its actual bound address (from listener.local_addr()), so
port-scanning (TokioAdapter) and $PORT env-var rewriting (PortEnvAdapter)
are handled correctly. LambdaAdapter drops the callback.
```

Also update the `source_refs` entry for `start.rs` to:
```toml
  "crates/runtime/src/start.rs — on_bind closure passed to adapter.serve",
```

- [ ] **Step 5: Commit**

```bash
git add pilcrow/crates/runtime/src/adapter.rs \
        pilcrow/crates/runtime/src/adapters/server.rs \
        pilcrow/crates/runtime/src/adapters/lambda.rs \
        pilcrow/crates/runtime/src/start.rs \
        pilcrow/registry.toml
git commit -m "$(cat <<'EOF'
fix(prebake): derive local base from actual bound port via on_bind callback

PilcrowAdapter::serve() now takes an on_bind callback that adapters
call after binding, passing their actual local_addr(). TokioAdapter
(which port-scans) and PortEnvAdapter (which reads $PORT) both report
the real bound address. LambdaAdapter drops the callback since it has
no local HTTP server.

start_with_adapter passes a closure that normalises 0.0.0.0 to 127.0.0.1
and calls prebake::set_local_base, replacing the premature config-port
guess that was wrong whenever a port scan or $PORT redirect occurred.

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)"
```
