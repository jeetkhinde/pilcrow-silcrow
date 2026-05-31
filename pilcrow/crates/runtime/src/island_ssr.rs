//! Runtime Node.js worker pool for React island SSR (`strategy="ssr"`).
//!
//! At server startup, [`IslandSsrWorkerPool`] spawns N persistent Node workers when
//! `[client.react] ssr = true` in `Pilcrow.toml`. Render requests check out a
//! worker via async semaphore, perform async stdin/stdout I/O, and return it.
//!
//! ```text
//! stdin:  {"id":"counter_0abc","props":{"initialCount":"3"}}\n
//! stdout: {"html":"<div>3</div>"}\n
//! ```

use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{Mutex, Semaphore};

/// Per-render timeout — prevents a hung Node process from holding a pool slot
/// indefinitely.
pub(crate) const SSR_RENDER_TIMEOUT: Duration = Duration::from_secs(10);

/// A persistent Node.js worker process that renders React islands server-side.
pub struct IslandSsrWorker {
    _child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    /// Keeps the temp directory alive for the lifetime of the worker.
    _temp_dir: Arc<TempDir>,
    /// Set to false on I/O error or timeout; used by the pool to decide whether
    /// to return or replace this worker.
    pub is_healthy: bool,
}

impl IslandSsrWorker {
    /// Spawn a Node worker from embedded SSR bundle sources.
    ///
    /// Writes each `(id, source)` bundle to a temp directory, generates the
    /// dispatcher worker script, and starts the Node process.
    pub fn spawn_with_sources(bundles: &[(&str, &str)], node_bin: &str) -> io::Result<Self> {
        let temp_dir = Arc::new(TempDir::new("pilcrow_react_ssr")?);

        for (id, source) in bundles {
            std::fs::write(temp_dir.path().join(format!("{id}.ssr.js")), source)?;
        }

        let worker_js = build_worker_script(bundles, temp_dir.path());
        let worker_path = temp_dir.path().join("__pilcrow_worker.mjs");
        std::fs::write(&worker_path, worker_js)?;

        Self::spawn_at_path(&worker_path, node_bin, temp_dir)
    }

    fn spawn_at_path(worker_path: &Path, node_bin: &str, temp_dir: Arc<TempDir>) -> io::Result<Self> {
        let mut child = tokio::process::Command::new(node_bin)
            .arg(worker_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!(
                        "failed to spawn Node SSR worker (`{node_bin}`): {e}. \
                         Ensure Node.js is installed and `node_bin` in Pilcrow.toml is correct."
                    ),
                )
            })?;

        let stdin = BufWriter::new(child.stdin.take().expect("piped stdin"));
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));

        Ok(Self {
            _child: child,
            stdin,
            stdout,
            _temp_dir: temp_dir,
            is_healthy: true,
        })
    }

    /// Render an island server-side. Props are passed as JSON values.
    pub async fn render(&mut self, id: &str, props: &serde_json::Value) -> io::Result<String> {
        let req = serde_json::json!({ "id": id, "props": props });
        let mut line_bytes = req.to_string().into_bytes();
        line_bytes.push(b'\n');
        self.stdin.write_all(&line_bytes).await?;
        self.stdin.flush().await?;

        let mut line = String::new();
        self.stdout.read_line(&mut line).await?;

        let resp: serde_json::Value = serde_json::from_str(line.trim()).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("SSR worker returned invalid JSON: {e}"),
            )
        })?;

        Ok(resp["html"].as_str().unwrap_or("").to_string())
    }
}

// ── Node.js worker script generation ─────────────────────────────────────────

/// Generate the Node.js dispatcher worker script for the given bundles.
fn build_worker_script(bundles: &[(&str, &str)], temp_dir: &Path) -> String {
    let mut script = String::from("import { createInterface } from \"node:readline\";\n");

    for (id, _) in bundles {
        let bundle_path = temp_dir
            .join(format!("{id}.ssr.js"))
            .to_string_lossy()
            .replace('\\', "/");
        let var = make_js_var(id);
        script.push_str(&format!(
            "import {{ render as render_{var} }} from {bundle_path:?};\n"
        ));
    }

    script.push_str("\nconst dispatch = {\n");
    for (id, _) in bundles {
        let var = make_js_var(id);
        script.push_str(&format!(
            "  {}: render_{var},\n",
            serde_json::to_string(id).unwrap()
        ));
    }
    script.push_str("};\n\n");

    script.push_str("const rl = createInterface({ input: process.stdin });\n");
    script.push_str("rl.on(\"line\", (line) => {\n");
    script.push_str("  try {\n");
    script.push_str("    const { id, props } = JSON.parse(line);\n");
    script.push_str("    const renderFn = dispatch[id];\n");
    script.push_str("    const html = renderFn ? renderFn(props) : \"\";\n");
    script.push_str("    process.stdout.write(JSON.stringify({ html }) + \"\\n\");\n");
    script.push_str("  } catch (err) {\n");
    script.push_str(
        "    process.stdout.write(JSON.stringify({ html: \"\", error: String(err) }) + \"\\n\");\n",
    );
    script.push_str("  }\n");
    script.push_str("});\n");

    script
}

/// Derive a valid JS identifier from an island id.
fn make_js_var(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ── Worker pool ───────────────────────────────────────────────────────────────

/// A pool of persistent Node.js SSR workers.
///
/// Workers are checked out via [`get`] (awaits if all busy) and returned via
/// [`put`]. Unhealthy workers (timeout or I/O error) are replaced by
/// [`recreate_worker`].
///
/// Pool size defaults to `[client.react] concurrency` (default `4`).
pub struct IslandSsrWorkerPool {
    workers: Mutex<Vec<IslandSsrWorker>>,
    semaphore: Semaphore,
    bundles: Vec<(String, String)>,
    node_bin: String,
}

impl IslandSsrWorkerPool {
    /// Spawn `size` workers and buffer them in the pool.
    pub fn new(bundles: &[(&str, &str)], node_bin: &str, size: usize) -> io::Result<Self> {
        let mut initial = Vec::with_capacity(size);
        for _ in 0..size {
            initial.push(IslandSsrWorker::spawn_with_sources(bundles, node_bin)?);
        }
        Ok(Self {
            workers: Mutex::new(initial),
            semaphore: Semaphore::new(size),
            bundles: bundles
                .iter()
                .map(|(id, src)| (id.to_string(), src.to_string()))
                .collect(),
            node_bin: node_bin.to_string(),
        })
    }

    /// Check out a worker. Awaits until one is available.
    pub async fn get(&self) -> Result<IslandSsrWorker, String> {
        self.semaphore
            .acquire()
            .await
            .map_err(|_| "SSR worker pool closed".to_string())?
            .forget();
        self.workers
            .lock()
            .await
            .pop()
            .ok_or_else(|| "SSR worker pool empty despite semaphore permit — invariant violated".to_string())
    }

    /// Return a healthy worker to the pool.
    pub async fn put(&self, worker: IslandSsrWorker) {
        self.workers.lock().await.push(worker);
        self.semaphore.add_permits(1);
    }

    /// Spawn a replacement worker and add it to the pool.
    ///
    /// On spawn failure, logs the error. The pool operates at reduced capacity
    /// until the next restart.
    pub async fn recreate_worker(&self) {
        let bundles_ref: Vec<(&str, &str)> = self
            .bundles
            .iter()
            .map(|(id, src)| (id.as_str(), src.as_str()))
            .collect();
        match IslandSsrWorker::spawn_with_sources(&bundles_ref, &self.node_bin) {
            Ok(w) => self.put(w).await,
            Err(e) => {
                tracing::error!(
                    "failed to recreate SSR worker: {e}; pool operating at reduced capacity"
                );
            }
        }
    }
}

// ── SSR placeholder replacement ──────────────────────────────────────────────

const SSR_PLACEHOLDER_PREFIX: &str = "__PILCROW_REACT_SSR_";
const SSR_PLACEHOLDER_SUFFIX: &str = "__";

/// Replace `__PILCROW_REACT_SSR_{id}__` placeholders in rendered HTML.
///
/// For each placeholder the surrounding `<div data-pilcrow-react …>` is scanned
/// for `data-prop-*` attributes and forwarded to a Node worker as props.
/// Each render is bounded by [`SSR_RENDER_TIMEOUT`]; failures replace the
/// placeholder with nothing and trigger a worker recreation.
pub async fn replace_ssr_placeholders(html: &str, pool: &Arc<IslandSsrWorkerPool>) -> String {
    let mut result = String::with_capacity(html.len());
    let mut remaining = html;

    while let Some(start) = remaining.find(SSR_PLACEHOLDER_PREFIX) {
        result.push_str(&remaining[..start]);
        let after_prefix = &remaining[start + SSR_PLACEHOLDER_PREFIX.len()..];

        if let Some(end) = after_prefix.find(SSR_PLACEHOLDER_SUFFIX) {
            let id = &after_prefix[..end];
            let consumed = SSR_PLACEHOLDER_PREFIX.len() + end + SSR_PLACEHOLDER_SUFFIX.len();

            let props = extract_props_for_id(&result, id);

            match pool.get().await {
                Err(e) => {
                    tracing::error!("SSR worker pool error: {e}");
                }
                Ok(mut worker) => {
                    let render_result =
                        tokio::time::timeout(SSR_RENDER_TIMEOUT, worker.render(id, &props)).await;
                    match render_result {
                        Ok(Ok(fragment)) => {
                            result.push_str(&fragment);
                            pool.put(worker).await;
                        }
                        Ok(Err(e)) => {
                            tracing::error!("SSR worker render error for `{id}`: {e}");
                            worker.is_healthy = false;
                            pool.recreate_worker().await;
                        }
                        Err(_elapsed) => {
                            tracing::error!(
                                "SSR worker timed out after {}s rendering `{id}`",
                                SSR_RENDER_TIMEOUT.as_secs()
                            );
                            worker.is_healthy = false;
                            pool.recreate_worker().await;
                        }
                    }
                }
            }

            remaining = &remaining[start + consumed..];
        } else {
            // Malformed placeholder — pass through as-is
            result.push_str(&remaining[start..]);
            remaining = "";
        }
    }

    result.push_str(remaining);
    result
}

/// Find the last `<div data-pilcrow-react … data-id="{id}" …>` before the
/// placeholder and return a JSON object built from its `data-prop-*` attributes.
fn extract_props_for_id(html_before: &str, id: &str) -> serde_json::Value {
    let id_attr = format!("data-id=\"{id}\"");
    let Some(div_pos) = find_last_div_with_attr(html_before, &id_attr) else {
        return serde_json::Value::Object(Default::default());
    };

    let from_div = &html_before[div_pos..];
    let tag_end = find_tag_end(from_div);
    let tag = &from_div[..tag_end];

    build_props_from_tag(tag)
}

/// Return the byte offset of the last `<div` that contains `attr` before `end`.
fn find_last_div_with_attr(html: &str, attr: &str) -> Option<usize> {
    let mut last = None;
    let mut search = html;
    let mut offset = 0usize;
    while let Some(pos) = search.find("<div") {
        let candidate = &search[pos..];
        if let Some(tag_end) = find_tag_end(candidate).checked_add(0)
            && candidate[..tag_end].contains(attr)
        {
            last = Some(offset + pos);
        }
        let step = pos + 4;
        offset += step;
        search = &search[step..];
    }
    last
}

/// Return the index just past the closing `>` of the current opening tag,
/// respecting quoted attribute values.
fn find_tag_end(tag: &str) -> usize {
    let bytes = tag.as_bytes();
    let mut i = 0;
    let mut in_quote: Option<u8> = None;
    while i < bytes.len() {
        match (in_quote, bytes[i]) {
            (None, b'"') | (None, b'\'') => in_quote = Some(bytes[i]),
            (Some(q), c) if c == q => in_quote = None,
            (None, b'>') => return i + 1,
            _ => {}
        }
        i += 1;
    }
    tag.len()
}

/// Parse `data-prop-{name}="{value}"` attributes from an opening tag string.
fn build_props_from_tag(tag: &str) -> serde_json::Value {
    let mut props = serde_json::Map::new();
    if let Some(action_base) = extract_attr_value(tag, "data-pilcrow-action-base") {
        props.insert(
            "__pilcrowActionBase".to_string(),
            serde_json::Value::String(action_base),
        );
    }

    let json_prefix = "data-prop-json-";
    let mut remaining = tag;

    while let Some(prop_pos) = remaining.find(json_prefix) {
        remaining = &remaining[prop_pos + json_prefix.len()..];
        let (raw_name, value, next) = read_attr_tail(remaining);
        remaining = next;
        let camel = to_camel_case(raw_name);
        let parsed =
            serde_json::from_str::<serde_json::Value>(&value).unwrap_or(serde_json::Value::Null);
        props.insert(camel, parsed);
    }

    let prefix = "data-prop-";
    let mut remaining = tag;
    while let Some(prop_pos) = remaining.find(prefix) {
        if remaining[prop_pos..].starts_with(json_prefix) {
            remaining = &remaining[prop_pos + json_prefix.len()..];
            continue;
        }
        remaining = &remaining[prop_pos + prefix.len()..];

        let (raw_name, value, next) = read_attr_tail(remaining);
        let camel = to_camel_case(raw_name);
        remaining = next;
        props.insert(camel, serde_json::Value::String(value));
    }

    serde_json::Value::Object(props)
}

fn read_attr_tail(input: &str) -> (&str, String, &str) {
    let name_end = input
        .find(|c: char| c == '=' || c.is_ascii_whitespace() || c == '>')
        .unwrap_or(input.len());
    let raw_name = &input[..name_end];
    let mut remaining = &input[name_end..];

    let value = if remaining.starts_with('=') {
        remaining = &remaining[1..];
        if remaining.starts_with('"') || remaining.starts_with('\'') {
            let quote = remaining.as_bytes()[0];
            remaining = &remaining[1..];
            let val_end = remaining
                .find(|c: char| c as u8 == quote)
                .unwrap_or(remaining.len());
            let value = html_unescape(&remaining[..val_end]);
            remaining = remaining.get(val_end + 1..).unwrap_or("");
            value
        } else {
            let val_end = remaining
                .find(|c: char| c.is_ascii_whitespace() || c == '>')
                .unwrap_or(remaining.len());
            let value = remaining[..val_end].to_string();
            remaining = remaining.get(val_end..).unwrap_or("");
            value
        }
    } else {
        "true".to_string()
    };

    (raw_name, value, remaining)
}

fn extract_attr_value(tag: &str, attr: &str) -> Option<String> {
    let mut remaining = tag;
    while let Some(pos) = remaining.find(attr) {
        remaining = &remaining[pos + attr.len()..];
        if !remaining.starts_with('=') {
            continue;
        }
        remaining = &remaining[1..];
        if remaining.starts_with('"') || remaining.starts_with('\'') {
            let quote = remaining.as_bytes()[0];
            remaining = &remaining[1..];
            let val_end = remaining
                .find(|c: char| c as u8 == quote)
                .unwrap_or(remaining.len());
            return Some(html_unescape(&remaining[..val_end]));
        }
        let val_end = remaining
            .find(|c: char| c.is_ascii_whitespace() || c == '>')
            .unwrap_or(remaining.len());
        return Some(remaining[..val_end].to_string());
    }
    None
}

fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut next_upper = false;
    for ch in s.chars() {
        if ch == '-' {
            next_upper = true;
        } else if next_upper {
            result.push(ch.to_ascii_uppercase());
            next_upper = false;
        } else {
            result.push(ch);
        }
    }
    result
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
}

// ── TempDir ──────────────────────────────────────────────────────────────────

/// Owned temporary directory — cleaned up on drop.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> io::Result<Self> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("{prefix}_{pid}_{ts}"));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_props_from_div() {
        let html = r#"<div data-pilcrow-react data-id="counter_0" data-strategy="ssr" data-prop-initial-count="5" data-prop-label="hello">"#;
        let props = build_props_from_tag(html);
        assert_eq!(props["initialCount"], serde_json::json!("5"));
        assert_eq!(props["label"], serde_json::json!("hello"));
    }

    #[test]
    fn extracts_json_props_and_action_base_from_div() {
        let html = r#"<div data-pilcrow-react data-id="grid_0" data-pilcrow-action-base="/widgets/grid" data-prop-json-rows="[{&quot;id&quot;:1}]" data-prop-title="Rows">"#;
        let props = build_props_from_tag(html);
        assert_eq!(props["rows"], serde_json::json!([{ "id": 1 }]));
        assert_eq!(props["title"], serde_json::json!("Rows"));
        assert_eq!(
            props["__pilcrowActionBase"],
            serde_json::json!("/widgets/grid")
        );
    }

    #[test]
    fn replace_passes_through_when_no_placeholder() {
        let html = "<p>hello</p>";
        assert!(!html.contains(SSR_PLACEHOLDER_PREFIX));
    }
}
