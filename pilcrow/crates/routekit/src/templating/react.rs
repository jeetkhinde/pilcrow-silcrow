use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;

use crate::routing::discovery::IgnoreFilter;
use crate::templating::build_config::{ReactBuildConfig, RoutingConfig};

pub const REACT_ENTRY_PLACEHOLDER_PREFIX: &str = "__PILCROW_REACT_ENTRY_";
pub const REACT_CSS_PLACEHOLDER_PREFIX: &str = "__PILCROW_REACT_CSS_";
pub const REACT_SHELL_PLACEHOLDER_PREFIX: &str = "__PILCROW_REACT_SHELL_";
pub const REACT_SSR_PLACEHOLDER_PREFIX: &str = "__PILCROW_REACT_SSR_";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactIslandRef {
    pub id: String,
    pub source_path: PathBuf,
    /// The strategy attribute value: "load", "idle", "visible", "shell", or "ssr".
    pub strategy: String,
}

#[derive(Debug, Clone)]
struct ReactSource {
    id: String,
    source_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ViteManifestEntry {
    file: String,
    #[serde(default)]
    css: Vec<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    src: Option<String>,
    #[serde(default, rename = "isEntry")]
    is_entry: bool,
}

pub fn transpile_react_tags(
    template: &str,
    template_path: &Path,
    src_root: &Path,
    react_config: &ReactBuildConfig,
    routing: &RoutingConfig,
    action_base: Option<&str>,
) -> io::Result<(String, Vec<ReactIslandRef>)> {
    let mut output = String::with_capacity(template.len());
    let mut refs = Vec::new();
    let mut i = 0usize;
    let mut counter = 0usize;

    while i < template.len() {
        if let Some(consumed) = copy_html_comment(template, i, &mut output) {
            i += consumed;
            continue;
        }

        if template[i..].starts_with("<react") {
            let rest = &template[i + 6..];
            let next = rest.chars().next();
            if matches!(next, Some(c) if c.is_whitespace() || c == '/' || c == '>') {
                let (html, consumed, island) = parse_react_tag(
                    &template[i..],
                    template_path,
                    src_root,
                    react_config,
                    routing,
                    action_base,
                    counter,
                )?;
                output.push_str(&html);
                refs.push(island);
                i += consumed;
                counter += 1;
                continue;
            }
        }
        let c = template[i..].chars().next().unwrap();
        output.push(c);
        i += c.len_utf8();
    }

    Ok((output, refs))
}

fn copy_html_comment(input: &str, start: usize, output: &mut String) -> Option<usize> {
    if !input[start..].starts_with("<!--") {
        return None;
    }

    let consumed = match input[start + 4..].find("-->") {
        Some(end) => 4 + end + 3,
        None => input.len() - start,
    };
    output.push_str(&input[start..start + consumed]);
    Some(consumed)
}

fn parse_react_tag(
    input: &str,
    template_path: &Path,
    src_root: &Path,
    react_config: &ReactBuildConfig,
    routing: &RoutingConfig,
    action_base: Option<&str>,
    tag_index: usize,
) -> io::Result<(String, usize, ReactIslandRef)> {
    debug_assert!(input.starts_with("<react"));

    let (raw_attrs, consumed) = consume_tag_attrs(input, "react")?;
    let attrs = parse_attrs(&raw_attrs);
    let src = required_attr(&attrs, "src", template_path)?;
    let strategy = required_attr(&attrs, "strategy", template_path)?;
    if !matches!(
        strategy.as_str(),
        "load" | "visible" | "idle" | "shell" | "ssr"
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "`strategy` on <react> in {} must be one of: load, visible, idle, shell, ssr",
                template_path.display()
            ),
        ));
    }
    if matches!(strategy.as_str(), "shell" | "ssr") && !react_config.ssr {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "`strategy=\"{strategy}\"` on <react> in {} requires `[client.react] ssr = true` in Pilcrow.toml",
                template_path.display()
            ),
        ));
    }

    let source_path = resolve_react_source(&src, template_path, src_root)?;
    if !source_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "<react src=\"{}\"> in {} resolved to {}, but that file does not exist",
                src,
                template_path.display(),
                source_path.display()
            ),
        ));
    }

    validate_react_source_directory(&source_path, src_root, react_config, routing)?;

    let id = react_id(template_path, tag_index);
    let entry_placeholder = format!("{REACT_ENTRY_PLACEHOLDER_PREFIX}{id}__");
    let css_placeholder = format!("{REACT_CSS_PLACEHOLDER_PREFIX}{id}__");
    let mut html = format!(
        "<div data-pilcrow-react data-id=\"{}\" data-src=\"{}\" data-strategy=\"{}\" data-css=\"{}\"",
        html_escape_attr(&id),
        html_escape_attr(&entry_placeholder),
        html_escape_attr(&strategy),
        html_escape_attr(&css_placeholder),
    );
    if let Some(action_base) = action_base {
        html.push_str(&format!(
            " data-pilcrow-action-base=\"{}\" data-prop-__pilcrow-action-base=\"{}\"",
            html_escape_attr(action_base),
            html_escape_attr(action_base)
        ));
    }

    for (name, value) in &attrs {
        if name == "src" || name == "strategy" {
            continue;
        }
        let (attr_prefix, prop_name) = if let Some(prop_name) = name.strip_prefix("json-") {
            ("data-prop-json-", prop_name)
        } else {
            ("data-prop-", name.as_str())
        };
        if value.is_empty() {
            html.push_str(&format!(
                " {attr_prefix}{}=\"true\"",
                html_escape_attr(prop_name)
            ));
        } else {
            html.push_str(&format!(
                " {attr_prefix}{}=\"{}\"",
                html_escape_attr(prop_name),
                html_escape_attr(value)
            ));
        }
    }

    // Close the opening tag and embed the strategy-specific placeholder or leave empty.
    match strategy.as_str() {
        "shell" => html.push_str(&format!(">{REACT_SHELL_PLACEHOLDER_PREFIX}{id}__</div>")),
        "ssr" => html.push_str(&format!(">{REACT_SSR_PLACEHOLDER_PREFIX}{id}__</div>")),
        _ => html.push_str("></div>"),
    }

    html.push_str("<script type=\"module\" src=\"{{ pilcrow_web::assets::assets::react_islands_js_path() }}\"></script>");

    Ok((
        html,
        consumed,
        ReactIslandRef {
            id,
            source_path,
            strategy,
        },
    ))
}

/// Build all React island assets. Returns:
/// - `id_to_urls`: island-id → (JS entry URL, CSS URLs) for entry/CSS placeholder replacement
/// - `shell_html`: island-id → static shell HTML for shell placeholder replacement (build time)
#[allow(clippy::type_complexity)]
pub fn build_react_assets(
    manifest_dir: &Path,
    out_dir: &Path,
    islands: &[ReactIslandRef],
    react_config: &ReactBuildConfig,
) -> io::Result<(
    HashMap<String, (String, Vec<String>)>,
    HashMap<String, String>,
)> {
    write_empty_react_assets(out_dir)?;
    write_empty_react_ssr(out_dir)?;
    write_empty_react_shells(out_dir)?;

    if islands.is_empty() {
        return Ok((HashMap::new(), HashMap::new()));
    }
    if !react_config.enabled {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "<react> tags were found, but [client.react].enabled is not true in Pilcrow.toml",
        ));
    }

    let mut sources_by_id = BTreeMap::new();
    for island in islands {
        sources_by_id
            .entry(island.id.clone())
            .or_insert_with(|| ReactSource {
                id: island.id.clone(),
                source_path: island.source_path.clone(),
            });
    }

    let react_root = out_dir.join("pilcrow_react");
    let entries_dir = react_root.join("entries");
    let dist_dir = react_root.join("dist");
    fs::create_dir_all(&entries_dir)?;
    fs::create_dir_all(&dist_dir)?;
    let support_module = write_react_support_module(&react_root)?;

    // ── Client bundle ────────────────────────────────────────────────────────
    let mut inputs = BTreeMap::new();
    for source in sources_by_id.values() {
        let entry_path = entries_dir.join(format!("{}.tsx", source.id));
        fs::write(
            &entry_path,
            render_entry_wrapper(&source.id, &source.source_path),
        )?;
        inputs.insert(source.id.clone(), entry_path);
    }

    let config_path = react_root.join("vite.config.mjs");
    fs::write(
        &config_path,
        render_client_vite_config(&inputs, &dist_dir, &support_module),
    )?;
    run_vite(manifest_dir, &config_path)?;

    let manifest_path = dist_dir.join(".vite/manifest.json");
    let manifest_raw = fs::read_to_string(&manifest_path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "React island build completed but {} could not be read: {err}",
                manifest_path.display()
            ),
        )
    })?;
    let manifest: HashMap<String, ViteManifestEntry> = serde_json::from_str(&manifest_raw)
        .map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "failed to parse Vite manifest {}: {err}",
                    manifest_path.display()
                ),
            )
        })?;

    let mut id_to_urls = HashMap::new();
    for source in sources_by_id.values() {
        let manifest_key = format!("{}.tsx", source.id);
        let entry = manifest
            .get(&manifest_key)
            .or_else(|| {
                manifest.iter().find_map(|(key, entry)| {
                    let key_matches = key.ends_with(&format!("/{}", manifest_key));
                    let src_matches = entry
                        .src
                        .as_deref()
                        .is_some_and(|src| src.ends_with(&format!("/{}", manifest_key)));
                    let name_matches = entry.name.as_deref() == Some(source.id.as_str());
                    if entry.is_entry && (key_matches || src_matches || name_matches) {
                        Some(entry)
                    } else {
                        None
                    }
                })
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Vite manifest did not contain React island entry `{manifest_key}`"),
                )
            })?;
        id_to_urls.insert(
            source.id.clone(),
            (
                format!("/_pilcrow/client/{}", entry.file),
                entry
                    .css
                    .iter()
                    .map(|css| format!("/_pilcrow/client/{css}"))
                    .collect::<Vec<_>>(),
            ),
        );
    }

    check_entry_budgets(&dist_dir, &id_to_urls, react_config)?;
    write_react_assets_module(out_dir, &dist_dir)?;
    check_budgets(&dist_dir, react_config)?;

    // ── SSR bundle (for shell + ssr strategies) ──────────────────────────────
    let ssr_islands: Vec<&ReactIslandRef> = islands
        .iter()
        .filter(|r| r.strategy == "shell" || r.strategy == "ssr")
        .collect();

    if ssr_islands.is_empty() {
        return Ok((id_to_urls, HashMap::new()));
    }

    let mut ssr_sources_by_id: BTreeMap<String, ReactSource> = BTreeMap::new();
    for island in &ssr_islands {
        ssr_sources_by_id
            .entry(island.id.clone())
            .or_insert_with(|| ReactSource {
                id: island.id.clone(),
                source_path: island.source_path.clone(),
            });
    }

    // Generate server entry files (.server.tsx)
    let mut ssr_inputs = BTreeMap::new();
    for source in ssr_sources_by_id.values() {
        let entry_path = entries_dir.join(format!("{}.server.tsx", source.id));
        fs::write(
            &entry_path,
            render_server_entry_wrapper(&source.source_path),
        )?;
        ssr_inputs.insert(source.id.clone(), entry_path);
    }

    let dist_ssr_dir = react_root.join("dist_ssr");
    fs::create_dir_all(&dist_ssr_dir)?;

    let ssr_config_path = react_root.join("vite.ssr.config.mjs");
    fs::write(
        &ssr_config_path,
        render_ssr_vite_config(&ssr_inputs, &dist_ssr_dir, &support_module),
    )?;
    run_vite(manifest_dir, &ssr_config_path)?;

    write_react_ssr_module(out_dir, &dist_ssr_dir, &ssr_sources_by_id)?;

    // ── Static shells (for shell strategy only) ──────────────────────────────
    let mut shell_html: HashMap<String, String> = HashMap::new();

    for island in &ssr_islands {
        if island.strategy != "shell" {
            continue;
        }
        let bundle_path = dist_ssr_dir.join(format!("{}.ssr.js", island.id));
        match render_static_shell(
            &island.id,
            &bundle_path,
            manifest_dir,
            &react_config.node_bin,
        ) {
            Ok(html) => {
                shell_html.insert(island.id.clone(), html);
            }
            Err(err) => {
                // Non-fatal: emit a build warning; the island falls back to CSR
                println!(
                    "cargo:warning=Pilcrow: failed to render static shell for island `{}`: {err}",
                    island.id
                );
            }
        }
    }

    write_react_shells_module(out_dir, &shell_html)?;

    Ok((id_to_urls, shell_html))
}

pub fn replace_react_placeholders(
    html: &str,
    id_to_urls: &HashMap<String, (String, Vec<String>)>,
) -> String {
    let mut out = html.to_string();
    for (id, (entry, css)) in id_to_urls {
        out = out.replace(
            &format!("{REACT_ENTRY_PLACEHOLDER_PREFIX}{id}__"),
            entry.as_str(),
        );
        out = out.replace(
            &format!("{REACT_CSS_PLACEHOLDER_PREFIX}{id}__"),
            &css.join(","),
        );
    }
    out
}

/// Replace `__PILCROW_REACT_SHELL_{id}__` placeholders with pre-rendered shell HTML.
/// Called at build time after `build_react_assets` so static shells land in Askama templates.
pub fn replace_react_shell_placeholders(
    html: &str,
    shell_html: &HashMap<String, String>,
) -> String {
    let mut out = html.to_string();
    for (id, shell) in shell_html {
        out = out.replace(
            &format!("{REACT_SHELL_PLACEHOLDER_PREFIX}{id}__"),
            shell.as_str(),
        );
    }
    out
}

fn run_vite(manifest_dir: &Path, config_path: &Path) -> io::Result<()> {
    let vite_bin = if cfg!(windows) {
        manifest_dir.join("node_modules/.bin/vite.cmd")
    } else {
        manifest_dir.join("node_modules/.bin/vite")
    };
    if !vite_bin.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "React islands require local Vite dependencies. Add package.json dependencies for vite, react, react-dom, and run npm install in {}",
                manifest_dir.display()
            ),
        ));
    }

    let status = Command::new(vite_bin)
        .arg("build")
        .arg("--config")
        .arg(config_path)
        .current_dir(manifest_dir)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("React island Vite build failed"))
    }
}

/// Client-side entry wrapper: uses `hydrateRoot` when server-rendered HTML is present,
/// falls back to `createRoot` for pure CSR islands.
fn render_entry_wrapper(id: &str, source_path: &Path) -> String {
    let src = source_path.to_string_lossy().replace('\\', "/");
    let id_json = serde_json::to_string(id).unwrap();
    format!(
        r#"import React from "react";
import {{ createRoot, hydrateRoot }} from "react-dom/client";
import {{ PilcrowReactProvider }} from "pilcrow/react";
import Component from "{src}";

export function mount(el, props) {{
  const {{ __pilcrowActionBase, ...componentProps }} = props || {{}};
  const actionBase = __pilcrowActionBase || el.getAttribute("data-pilcrow-action-base") || window.location.pathname;
  const tree = React.createElement(
    PilcrowReactProvider,
    {{ value: {{ actionBase }} }},
    React.createElement(Component, componentProps),
  );
  if (el.__pilcrowReactRoot) {{
    el.__pilcrowReactRoot.render(tree);
    return;
  }}
  if (el.children.length > 0) {{
    // Server-rendered HTML present — hydrate instead of replacing
    hydrateRoot(el, tree);
    return;
  }}
  const root = createRoot(el);
  el.__pilcrowReactRoot = root;
  root.render(tree);
}}

window.__pilcrowReactMounts = window.__pilcrowReactMounts || {{}};
window.__pilcrowReactMounts[{id_json}] = mount;
"#
    )
}

/// Server-side entry wrapper: renders to an HTML string via `renderToString`.
fn render_server_entry_wrapper(source_path: &Path) -> String {
    let src = source_path.to_string_lossy().replace('\\', "/");
    format!(
        r#"import React from "react";
import {{ renderToString }} from "react-dom/server";
import {{ PilcrowReactProvider }} from "pilcrow/react";
import Component from "{src}";

export function render(props) {{
  const {{ __pilcrowActionBase, ...componentProps }} = props || {{}};
  const tree = React.createElement(
    PilcrowReactProvider,
    {{ value: {{ actionBase: __pilcrowActionBase || "/" }} }},
    React.createElement(Component, componentProps),
  );
  return renderToString(tree);
}}
"#
    )
}

fn render_client_vite_config(
    inputs: &BTreeMap<String, PathBuf>,
    dist_dir: &Path,
    support_module: &Path,
) -> String {
    let out = dist_dir.to_string_lossy().replace('\\', "/");
    let support = support_module.to_string_lossy().replace('\\', "/");
    let input = inputs
        .iter()
        .map(|(id, path)| {
            format!(
                "      {}: {},",
                serde_json::to_string(id).unwrap(),
                serde_json::to_string(&path.to_string_lossy().replace('\\', "/")).unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"import {{ createRequire }} from "node:module";

const require = createRequire(process.cwd() + "/package.json");

export default {{
  root: process.cwd(),
  resolve: {{
    alias: [
      {{ find: /^react$/, replacement: require.resolve("react") }},
      {{ find: /^react-dom$/, replacement: require.resolve("react-dom") }},
      {{ find: /^react-dom\/client$/, replacement: require.resolve("react-dom/client") }},
      {{ find: /^react\/jsx-runtime$/, replacement: require.resolve("react/jsx-runtime") }},
      {{ find: /^pilcrow\/react$/, replacement: {} }}
    ]
  }},
  esbuild: {{ jsx: "automatic" }},
  build: {{
    outDir: {},
    emptyOutDir: true,
    manifest: true,
    rollupOptions: {{
      input: {{
{}
      }},
      output: {{
        entryFileNames: "[name]-[hash].js",
        chunkFileNames: "chunks/[name]-[hash].js",
        assetFileNames: "assets/[name]-[hash][extname]",
        manualChunks(id) {{
          if (id.includes("node_modules/react")) return "react";
        }}
      }}
    }}
  }}
}};
"#,
        serde_json::to_string(&support).unwrap(),
        serde_json::to_string(&out).unwrap(),
        input
    )
}

fn render_ssr_vite_config(
    inputs: &BTreeMap<String, PathBuf>,
    dist_ssr_dir: &Path,
    support_module: &Path,
) -> String {
    let out = dist_ssr_dir.to_string_lossy().replace('\\', "/");
    let support = support_module.to_string_lossy().replace('\\', "/");
    let input = inputs
        .iter()
        .map(|(id, path)| {
            format!(
                "      {}: {},",
                serde_json::to_string(id).unwrap(),
                serde_json::to_string(&path.to_string_lossy().replace('\\', "/")).unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"import {{ createRequire }} from "node:module";

const require = createRequire(process.cwd() + "/package.json");

export default {{
  root: process.cwd(),
  resolve: {{
    alias: [
      {{ find: /^react$/, replacement: require.resolve("react") }},
      {{ find: /^react-dom$/, replacement: require.resolve("react-dom") }},
      {{ find: /^react-dom\/server$/, replacement: require.resolve("react-dom/server") }},
      {{ find: /^react\/jsx-runtime$/, replacement: require.resolve("react/jsx-runtime") }},
      {{ find: /^pilcrow\/react$/, replacement: {} }}
    ]
  }},
  esbuild: {{ jsx: "automatic" }},
  build: {{
    ssr: true,
    outDir: {},
    emptyOutDir: true,
    rollupOptions: {{
      input: {{
{}
      }},
      output: {{
        entryFileNames: "[name].ssr.js",
        format: "esm"
      }}
    }}
  }}
}};
"#,
        serde_json::to_string(&support).unwrap(),
        serde_json::to_string(&out).unwrap(),
        input
    )
}

fn write_react_support_module(react_root: &Path) -> io::Result<PathBuf> {
    let module_path = react_root.join("pilcrow-react.ts");
    fs::write(&module_path, PILCROW_REACT_TS)?;
    Ok(module_path)
}

const PILCROW_REACT_TS: &str = r#"import {
  createContext,
  use,
  useActionState,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
  createElement,
} from "react";

/**
 * Full response shape returned by `window.Silcrow.submit`.
 *
 * @example
 * type CreateState = {ok: boolean; message?: string};
 * const result: SilcrowSubmitResult<CreateState> =
 *   await window.Silcrow!.submit("/cart/add/1", {quantity: 1});
 * if (result.ok) console.log(result.data.message);
 */
export type SilcrowSubmitResult<T = unknown> = {
  ok: boolean;
  status: number;
  data: T;
  html: string | null;
  headers: Headers;
  mutationId?: string;
};

/**
 * Network options forwarded to `Silcrow.submit`.
 *
 * @example
 * submitSilcrow<CreateState>("/cart/add/1", {
 *   method: "POST",
 *   scope: "cart:add",
 *   headers: {"x-source": "react-island"},
 * });
 */
export type SilcrowSubmitOptions = {
  method?: string;
  scope?: string;
  headers?: Record<string, string>;
  optimistic?: {
    /** Atom scope to patch optimistically (must match a `s-bind` scope or atom). */
    scope: string;
    /** Data to apply immediately before the round-trip completes. */
    data: unknown;
    /** Stable client id for this mutation; auto-generated when omitted. */
    mutationId?: string;
  };
};

/**
 * Options for the tiny React 19 action wrapper.
 *
 * `permalink` is passed to React's `useActionState`; the other fields are
 * passed to Silcrow's submit transport.
 *
 * @example
 * useSilcrowAction<CreateState>(
 *   "/cart/add/1",
 *   {ok: true},
 *   {scope: "cart:add", permalink: "/cart"},
 * );
 */
export type SilcrowActionOptions = SilcrowSubmitOptions & {
  permalink?: string;
};

export type PilcrowReactContextValue = {
  actionBase?: string;
};

const PilcrowReactContext = createContext<PilcrowReactContextValue>({});

export function PilcrowReactProvider({
  value,
  children,
}: {
  value: PilcrowReactContextValue;
  children: ReactNode;
}) {
  return createElement(PilcrowReactContext.Provider, {value}, children);
}

function appendActionName(base: string, name: string): string {
  if (/^https?:\/\//.test(name) || name.startsWith("/") || name.startsWith("?/")) {
    return name;
  }
  const cleanBase = base || (typeof window !== "undefined" ? window.location.pathname : "/");
  const separator = cleanBase.includes("?") ? "&" : "?";
  return `${cleanBase}${separator}/${encodeURIComponent(name)}`;
}

export function resolvePilcrowAction(name: string, base?: string): string {
  return appendActionName(base ?? "", name);
}

/**
 * Browser global installed by `silcrow.js`.
 *
 * Use this directly when integrating with another React library, such as
 * React Hook Form. For simple native forms, prefer `submitSilcrow` or
 * `useSilcrowAction`.
 *
 * @example
 * const data = await window.Silcrow!.prefetch<ProductData>("/products");
 * const unsubscribe = window.Silcrow!.subscribe("route:/cart", () => {});
 * window.Silcrow!.publish("route:/cart", {count: 2});
 */
declare global {
  interface Window {
    Silcrow?: {
      subscribe?: (scope: string, fn: () => void) => () => void;
      snapshot?: <T = unknown>(scope: string) => T | undefined;
      publish?: (scope: string, data: unknown) => void;
      prefetch?: <T = unknown>(path: string) => Promise<T>;
      submit?: <T = unknown>(
        url: string,
        body?: BodyInit | object | null,
        options?: SilcrowSubmitOptions,
      ) => Promise<SilcrowSubmitResult<T>>;
      publishOptimistic?: (scope: string, data: unknown, mutationId: string) => void;
      confirmOptimistic?: (mutationId: string) => void;
      revertOptimistic?: (mutationId: string) => void;
    };
  }
}

/**
 * Subscribe a React component to a Silcrow atom scope.
 *
 * @example
 * type Cart = {count: number; total: string};
 * const cart = useSilcrowAtom<Cart>("route:/cart", {count: 0, total: "$0.00"});
 * return <span>{cart.count}</span>;
 */
export function useSilcrowAtom<T>(scope: string, fallback: T): T {
  return useSyncExternalStore<T>(
    (notify) => window.Silcrow?.subscribe?.(scope, notify) ?? (() => {}),
    () => window.Silcrow?.snapshot?.<T>(scope) ?? fallback,
    () => window.Silcrow?.snapshot?.<T>(scope) ?? fallback,
  );
}

/**
 * Patch a Silcrow atom scope.
 *
 * This accepts patch data, not an updater function.
 *
 * @example
 * publishSilcrowAtom("route:/cart", {count: 3});
 */
export function publishSilcrowAtom<T>(scope: string, data: T): void {
  window.Silcrow?.publish?.(scope, data);
}

/**
 * Prefetch a route and return Silcrow's memoized promise for React `use()`.
 *
 * @example
 * function Products() {
 *   const promise = useSilcrowPrefetch<ProductData>("/products");
 *   return <Suspense fallback={<p>Loading...</p>}><Rows promise={promise} /></Suspense>;
 * }
 */
export function useSilcrowPrefetch<T>(path: string): Promise<T> {
  return useMemo(
    () =>
      window.Silcrow?.prefetch?.<T>(path) ??
      Promise.reject(new Error("Silcrow is not loaded")),
    [path],
  );
}

/**
 * Read a route atom by path.
 *
 * @example
 * const products = useSilcrowRoute<ProductData>("/products", {items: []});
 */
export function useSilcrowRoute<T>(path: string, fallback: T): T {
  return useSilcrowAtom<T>(`route:${path}`, fallback);
}

/**
 * Create a React 19 form action backed by Silcrow transport.
 *
 * Use this with React's `useActionState` when you want the raw primitive.
 *
 * @example
 * const [state, action, pending] = useActionState<CreateState, FormData>(
 *   submitSilcrow<CreateState>("/cart/add/1"),
 *   {ok: true},
 * );
 * return <form action={action}><button disabled={pending}>Add</button></form>;
 */
export function submitSilcrow<T>(
  url: string,
  options?: SilcrowSubmitOptions,
) {
  return async function action(_prev: T, formData: FormData): Promise<T> {
    if (!window.Silcrow?.submit) {
      throw new Error("Silcrow is not loaded");
    }
    const result = await window.Silcrow.submit<T>(url, formData, {
      method: options?.method ?? "POST",
      scope: options?.scope,
      headers: options?.headers,
    });
    return result.data ?? ({ok: result.ok, status: result.status} as T);
  };
}

/**
 * Create an async submit callback for React Hook Form or other form libraries.
 *
 * Silcrow/Pilcrow stays responsible for transport; the form library owns
 * validation, dirty/touched state, focus, arrays, and nested fields.
 *
 * @example
 * const onSubmit = form.handleSubmit(async (values) => {
 *   const result = await silcrowSubmitHandler<CreateState, CartValues>(
 *     "/cart/add/1",
 *   )(values);
 *   if (!result.ok) form.setError("quantity", {message: "Invalid quantity"});
 * });
 */
export function silcrowSubmitHandler<Result = unknown, Values = object>(
  url: string,
  options?: SilcrowSubmitOptions,
) {
  return async function submit(values: Values): Promise<SilcrowSubmitResult<Result>> {
    if (!window.Silcrow?.submit) {
      throw new Error("Silcrow is not loaded");
    }
    return window.Silcrow.submit<Result>(url, values as object, {
      method: options?.method ?? "POST",
      scope: options?.scope,
      headers: options?.headers,
    });
  };
}

/**
 * Tiny React 19 convenience wrapper over `useActionState(submitSilcrow(...))`.
 *
 * This is intentionally not a form framework. For complex client-side form UX,
 * use React Hook Form and `silcrowSubmitHandler`.
 *
 * @example
 * type CreateState = {ok: boolean; message?: string; errors?: Record<string, string>};
 * const [state, action, pending] =
 *   useSilcrowAction<CreateState>("/cart/add/1");
 * return <form action={action}><button disabled={pending}>Add</button></form>;
 */
export function useSilcrowAction<State>(
  url: string,
  initialState = {ok: true} as State,
  options?: SilcrowActionOptions,
) {
  const submitOptions = options
    ? {method: options.method, scope: options.scope, headers: options.headers}
    : undefined;
  return useActionState<State, FormData>(
    submitSilcrow<State>(url, submitOptions),
    initialState,
    options?.permalink,
  );
}

/**
 * React 19 action wrapper that resolves a Pilcrow page/fragment named action.
 *
 * @example
 * const [state, action, pending] = usePilcrowNamedAction<CreateState>("add");
 */
export function usePilcrowNamedAction<State>(
  name: string,
  initialState = {ok: true} as State,
  options?: SilcrowActionOptions & {base?: string},
) {
  const context = useContext(PilcrowReactContext);
  const url = resolvePilcrowAction(name, options?.base ?? context.actionBase);
  return useSilcrowAction<State>(url, initialState, options);
}

/**
 * Prefetch a route, suspend until ready, then subscribe to live updates.
 *
 * Requires a `<Suspense>` boundary above the component that calls this hook.
 *
 * @example
 * function Products() {
 *   const products = useSilcrowResource<ProductData>("/products", {items: []});
 *   return <ul>{products.items.map(p => <li key={p.id}>{p.name}</li>)}</ul>;
 * }
 */
export function useSilcrowResource<T>(path: string, fallback: T): T {
  const initial = use(useSilcrowPrefetch<T>(path));
  return useSilcrowRoute<T>(path, initial ?? fallback);
}

/**
 * Shared form state shape expected by `useSilcrowForm`.
 *
 * Servers should return `{ok, message?, errors?}` to use this hook.
 */
export type SilcrowFormState = {
  ok: boolean;
  message?: string;
  errors?: Record<string, string>;
};

export type SilcrowFormResult<State extends SilcrowFormState> = {
  state: State;
  action: (formData: FormData) => void;
  pending: boolean;
  ok: State["ok"];
  message: State["message"];
  errors: State["errors"];
};

/**
 * Object-style wrapper over `useSilcrowAction` for simple native forms.
 *
 * Use this when the tuple from `useSilcrowAction` is noisy in JSX. For complex
 * client-side form UX (validation, arrays, focus), use React Hook Form with
 * `silcrowSubmitHandler` instead.
 *
 * @example
 * type CreateState = {ok: boolean; message?: string; errors?: Record<string, string>};
 * const form = useSilcrowForm<CreateState>("/cart/add/1");
 * return (
 *   <form action={form.action}>
 *     <button disabled={form.pending}>Add</button>
 *     {form.message ? <p role="status">{form.message}</p> : null}
 *     {form.errors?.quantity ? <p role="alert">{form.errors.quantity}</p> : null}
 *   </form>
 * );
 */
export function useSilcrowForm<State extends SilcrowFormState = SilcrowFormState>(
  url: string,
  initialState = {ok: true} as State,
  options?: SilcrowActionOptions,
): SilcrowFormResult<State> {
  const [state, action, pending] = useSilcrowAction<State>(url, initialState, options);
  return useMemo(
    () => ({state, action, pending, ok: state.ok, message: state.message, errors: state.errors}),
    [state, action, pending],
  );
}

/**
 * Options for `useSilcrowMutation`.
 */
export type SilcrowMutationOptions<Data = unknown> = {
  /** URL to POST to. */
  url: string;
  /** HTTP method (default: `"POST"`). */
  method?: string;
  /** Extra request headers. */
  headers?: Record<string, string>;
  /** Optimistic update to apply immediately before the round-trip. */
  optimistic?: {
    scope: string;
    data: Data;
    mutationId?: string;
  };
  /** Called when the server confirms success. */
  onSuccess?: (result: SilcrowSubmitResult) => void;
  /** Called when the server returns an error response or network failure. */
  onError?: (error: unknown) => void;
};

/**
 * State returned by `useSilcrowMutation`.
 */
export type SilcrowMutationState<Data = unknown> = {
  mutate: (body?: BodyInit | object | null) => Promise<SilcrowSubmitResult>;
  pending: boolean;
  error: unknown;
  data: Data | null;
  reset: () => void;
};

/**
 * A simple mutation hook that wraps `Silcrow.submit` with optional optimistic updates.
 *
 * Unlike `useSilcrowAction`, this hook does not use React 19 `useActionState`.
 * It is suitable for mutations triggered by event handlers (button clicks, drag-and-drop)
 * where you need full control over when and how the mutation fires.
 *
 * @example
 * type CartItem = {count: number};
 * const {mutate, pending, error} = useSilcrowMutation<CartItem>({
 *   url: "/cart/add/1",
 *   optimistic: {scope: "route:/cart", data: {count: prev.count + 1}},
 *   onSuccess: (result) => console.log("Added to cart", result.data),
 * });
 * return <button onClick={() => mutate()} disabled={pending}>Add to cart</button>;
 */
export function useSilcrowMutation<Data = unknown>(
  options: SilcrowMutationOptions<Data>,
): SilcrowMutationState<Data> {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [data, setData] = useState<Data | null>(null);

  const optionsRef = useRef(options);
  optionsRef.current = options;

  const reset = useCallback(() => {
    setPending(false);
    setError(null);
    setData(null);
  }, []);

  const mutate = useCallback(async (body?: BodyInit | object | null) => {
    const {url, method, headers, optimistic, onSuccess, onError} = optionsRef.current;
    if (!window.Silcrow?.submit) {
      const err = new Error("Silcrow is not loaded");
      setError(err);
      onError?.(err);
      throw err;
    }
    setPending(true);
    setError(null);
    try {
      const result = await window.Silcrow.submit(url, body ?? null, {
        method: method ?? "POST",
        headers,
        optimistic,
      });
      setData(result.data as Data);
      if (result.ok) {
        onSuccess?.(result);
      } else {
        const err = new Error("Request failed with status " + result.status);
        setError(err);
        onError?.(err);
      }
      return result;
    } catch (err) {
      setError(err);
      onError?.(err);
      throw err;
    } finally {
      setPending(false);
    }
  }, []);

  return {mutate, pending, error, data, reset};
}
"#;

fn write_empty_react_assets(out_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(out_dir)?;
    fs::write(
        out_dir.join("generated_react_assets.rs"),
        r#"pub fn asset(_path: &str) -> Option<(&'static str, &'static [u8])> {
    None
}
"#,
    )
}

fn write_empty_react_ssr(out_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(out_dir)?;
    fs::write(
        out_dir.join("generated_react_ssr.rs"),
        r#"pub const SSR_BUNDLES: &[(&str, &str)] = &[];

pub fn ssr_bundle(_id: &str) -> Option<&'static str> {
    None
}
"#,
    )
}

fn write_empty_react_shells(out_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(out_dir)?;
    fs::write(
        out_dir.join("generated_react_shells.rs"),
        r#"pub fn shell(_id: &str) -> Option<&'static str> {
    None
}
"#,
    )
}

fn write_react_assets_module(out_dir: &Path, dist_dir: &Path) -> io::Result<()> {
    let mut files = Vec::new();
    collect_dist_files(dist_dir, dist_dir, &mut files)?;
    files.sort();

    let mut src = String::new();
    src.push_str("pub fn asset(path: &str) -> Option<(&'static str, &'static [u8])> {\n");
    src.push_str("    match path {\n");
    for rel in files {
        let rel_text = rel.to_string_lossy().replace('\\', "/");
        if rel_text == ".vite/manifest.json" {
            continue;
        }
        let abs = dist_dir.join(&rel);
        let abs_text = abs.to_string_lossy().replace('\\', "/");
        let content_type = content_type_for(&rel_text);
        src.push_str(&format!(
            "        {:?} => Some(({:?}, include_bytes!({:?}) as &'static [u8])),\n",
            rel_text, content_type, abs_text
        ));
    }
    src.push_str("        _ => None,\n");
    src.push_str("    }\n");
    src.push_str("}\n");
    fs::write(out_dir.join("generated_react_assets.rs"), src)
}

fn write_react_ssr_module(
    out_dir: &Path,
    dist_ssr_dir: &Path,
    sources: &BTreeMap<String, ReactSource>,
) -> io::Result<()> {
    let mut src = String::new();

    src.push_str("pub const SSR_BUNDLES: &[(&str, &str)] = &[\n");
    for source in sources.values() {
        let bundle_path = dist_ssr_dir.join(format!("{}.ssr.js", source.id));
        if bundle_path.exists() {
            let abs_text = bundle_path.to_string_lossy().replace('\\', "/");
            src.push_str(&format!(
                "    ({:?}, include_str!({:?})),\n",
                source.id, abs_text
            ));
        }
    }
    src.push_str("];\n\n");

    src.push_str("pub fn ssr_bundle(id: &str) -> Option<&'static str> {\n");
    src.push_str("    SSR_BUNDLES.iter().find(|(k, _)| *k == id).map(|(_, v)| *v)\n");
    src.push_str("}\n");

    fs::write(out_dir.join("generated_react_ssr.rs"), src)
}

fn write_react_shells_module(out_dir: &Path, shells: &HashMap<String, String>) -> io::Result<()> {
    let mut src = String::new();
    src.push_str("pub fn shell(id: &str) -> Option<&'static str> {\n");
    if shells.is_empty() {
        src.push_str("    let _ = id;\n");
        src.push_str("    None\n");
    } else {
        src.push_str("    match id {\n");
        for (id, html) in shells {
            src.push_str(&format!("        {:?} => Some({:?}),\n", id, html));
        }
        src.push_str("        _ => None,\n");
        src.push_str("    }\n");
    }
    src.push_str("}\n");
    fs::write(out_dir.join("generated_react_shells.rs"), src)
}

/// Run the component with empty props via Node to capture the static shell HTML.
/// Used at build time for `strategy="shell"` islands.
fn render_static_shell(
    id: &str,
    ssr_bundle_path: &Path,
    manifest_dir: &Path,
    node_bin: &str,
) -> io::Result<String> {
    let runner_path = ssr_bundle_path
        .parent()
        .unwrap_or(manifest_dir)
        .join(format!("__pilcrow_shell_{id}.mjs"));

    let bundle_url = ssr_bundle_path.to_string_lossy().replace('\\', "/");
    let script =
        format!("import {{ render }} from {bundle_url:?};\nprocess.stdout.write(render({{}}));\n");
    fs::write(&runner_path, &script)?;

    let result = Command::new(node_bin)
        .arg(&runner_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(manifest_dir)
        .output();

    let _ = fs::remove_file(&runner_path);

    let output = result?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!(
            "Node shell render failed for island `{id}`: {stderr}"
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn collect_dist_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_dist_files(root, &path, out)?;
        } else if file_type.is_file() {
            out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        }
    }
    Ok(())
}

fn check_budgets(dist_dir: &Path, react_config: &ReactBuildConfig) -> io::Result<()> {
    if !react_config.warn && !react_config.fail_on_budget {
        return Ok(());
    }
    let mut files = Vec::new();
    collect_dist_files(dist_dir, dist_dir, &mut files)?;
    let total_js = files
        .iter()
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("js"))
        .filter_map(|rel| fs::metadata(dist_dir.join(rel)).ok())
        .map(|meta| meta.len())
        .sum::<u64>();
    if let Some(limit_kb) = react_config.max_page_react_kb {
        let limit = limit_kb * 1024;
        if total_js > limit {
            let msg = format!(
                "React island JS is {} KB, above max_page_react_kb = {}",
                total_js / 1024,
                limit_kb
            );
            if react_config.fail_on_budget {
                return Err(io::Error::other(msg));
            }
            println!("cargo:warning={msg}");
        }
    }
    Ok(())
}

fn check_entry_budgets(
    dist_dir: &Path,
    id_to_urls: &HashMap<String, (String, Vec<String>)>,
    react_config: &ReactBuildConfig,
) -> io::Result<()> {
    if !react_config.warn && !react_config.fail_on_budget {
        return Ok(());
    }
    let Some(limit_kb) = react_config.max_island_kb else {
        return Ok(());
    };
    let limit = limit_kb * 1024;
    for (id, (entry_url, _)) in id_to_urls {
        let rel = entry_url.trim_start_matches("/_pilcrow/client/");
        let size = fs::metadata(dist_dir.join(rel))
            .map(|meta| meta.len())
            .unwrap_or(0);
        if size > limit {
            let msg = format!(
                "React island `{id}` entry is {} KB, above max_island_kb = {}",
                size / 1024,
                limit_kb
            );
            if react_config.fail_on_budget {
                return Err(io::Error::other(msg));
            }
            println!("cargo:warning={msg}");
        }
    }
    Ok(())
}

fn content_type_for(path: &str) -> &'static str {
    if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}

fn resolve_react_source(src: &str, template_path: &Path, src_root: &Path) -> io::Result<PathBuf> {
    let path = if src.starts_with('/') {
        src_root.join(src.trim_start_matches('/'))
    } else {
        template_path.parent().unwrap_or(src_root).join(src)
    };
    path.canonicalize().or_else(|_| Ok(normalize_path(path)))
}

fn validate_react_source_directory(
    source_path: &Path,
    src_root: &Path,
    react_config: &ReactBuildConfig,
    routing: &RoutingConfig,
) -> io::Result<()> {
    let canonical_src_root = src_root
        .canonicalize()
        .unwrap_or_else(|_| normalize_path(src_root.to_path_buf()));
    let canonical_source = source_path
        .canonicalize()
        .unwrap_or_else(|_| normalize_path(source_path.to_path_buf()));
    let source_rel = canonical_source
        .strip_prefix(&canonical_src_root)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "React source {} must live under {}",
                    canonical_source.display(),
                    canonical_src_root.display()
                ),
            )
        })?;
    let filter = IgnoreFilter::new(&routing.ignore_directories);
    let dirs = if react_config.dirs.is_empty() {
        vec!["react".to_string()]
    } else {
        react_config.dirs.clone()
    };
    let mut current = canonical_source.parent();
    while let Some(dir) = current {
        if dir == canonical_src_root {
            break;
        }
        let Some(name) = dir.file_name().and_then(|name| name.to_str()) else {
            current = dir.parent();
            continue;
        };
        if dirs.iter().any(|allowed| allowed == name) {
            let rel = dir.strip_prefix(&canonical_src_root).unwrap_or(dir);
            if filter.is_ignored(rel) {
                return Ok(());
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "React source {} is in `{name}`, but that directory is not ignored by [routing].ignore_directories. Add `ignore_directories = [\"{name}\"]`.",
                    source_rel.display()
                ),
            ));
        }
        current = dir.parent();
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "React source {} must live inside one of [client.react].dirs: {:?}",
            source_rel.display(),
            dirs
        ),
    ))
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn react_id(template_path: &Path, tag_index: usize) -> String {
    let raw = format!("{}_{}", template_path.to_string_lossy(), tag_index);
    let mut out = String::from("react");
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push('_');
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_string()
}

fn required_attr(
    attrs: &HashMap<String, String>,
    name: &str,
    template_path: &Path,
) -> io::Result<String> {
    attrs
        .get(name)
        .filter(|v| !v.is_empty())
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("<react> in {} requires `{name}`", template_path.display()),
            )
        })
}

fn consume_tag_attrs(input: &str, tag: &str) -> io::Result<(String, usize)> {
    let mut idx = tag.len() + 1;
    let mut raw_attrs = String::new();
    let mut quote: Option<char> = None;
    let mut brace_depth = 0usize;
    while idx < input.len() {
        let c = input[idx..].chars().next().unwrap();
        let c_len = c.len_utf8();
        if let Some(q) = quote {
            raw_attrs.push(c);
            if c == q {
                quote = None;
            }
            idx += c_len;
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                raw_attrs.push(c);
                idx += c_len;
            }
            '{' => {
                brace_depth += 1;
                raw_attrs.push(c);
                idx += c_len;
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                raw_attrs.push(c);
                idx += c_len;
            }
            '/' if brace_depth == 0 => {
                idx += c_len;
                if input[idx..].starts_with('>') {
                    return Ok((raw_attrs, idx + 1));
                }
                raw_attrs.push('/');
            }
            '>' if brace_depth == 0 => return Ok((raw_attrs, idx + c_len)),
            _ => {
                raw_attrs.push(c);
                idx += c_len;
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "unterminated <react> tag",
    ))
}

fn parse_attrs(raw: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let mut i = 0usize;
    let bytes = raw.as_bytes();
    while i < raw.len() {
        while i < raw.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= raw.len() {
            break;
        }
        let name_start = i;
        while i < raw.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'/'
        {
            i += 1;
        }
        let name = raw[name_start..i].trim().to_string();
        while i < raw.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= raw.len() || bytes[i] != b'=' {
            attrs.insert(name, String::new());
            continue;
        }
        i += 1;
        while i < raw.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if i < raw.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
            let quote = bytes[i];
            i += 1;
            let value_start = i;
            while i < raw.len() && bytes[i] != quote {
                i += 1;
            }
            let value = raw[value_start..i].to_string();
            if i < raw.len() {
                i += 1;
            }
            value
        } else {
            let value_start = i;
            while i < raw.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            raw[value_start..i].to_string()
        };
        attrs.insert(name, value);
    }
    attrs
}

fn html_escape_attr(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templating::build_config::{ReactBuildConfig, RoutingConfig};

    #[test]
    fn react_tag_transpiles_to_inert_mount() {
        let root = std::env::temp_dir().join("pilcrow_react_tag_transpile");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/pages/dashboard/react")).unwrap();
        let page = root.join("src/pages/dashboard/index.html");
        fs::write(
            root.join("src/pages/dashboard/react/Counter.tsx"),
            "export default function Counter() { return null }",
        )
        .unwrap();

        let (out, refs) = transpile_react_tags(
            r#"<react src="./react/Counter.tsx" strategy="visible" count="{{ props.count }}" />"#,
            &page,
            &root.join("src"),
            &ReactBuildConfig {
                enabled: true,
                dirs: vec!["react".into()],
                ..Default::default()
            },
            &RoutingConfig {
                ignore_directories: vec!["react".into()],
            },
            Some("/dashboard"),
        )
        .unwrap();

        assert!(out.contains("data-pilcrow-react"));
        assert!(out.contains("data-strategy=\"visible\""));
        assert!(out.contains("data-prop-count=\"{{ props.count }}\""));
        assert!(out.contains("data-pilcrow-action-base=\"/dashboard\""));
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].strategy, "visible");
    }

    #[test]
    fn react_tag_ignores_tags_inside_html_comments() {
        let root = std::env::temp_dir().join("pilcrow_react_tag_comment");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/pages/react")).unwrap();
        let page = root.join("src/pages/index.html");
        fs::write(
            root.join("src/pages/react/Visible.tsx"),
            "export default function Visible() { return null }",
        )
        .unwrap();

        let input = r#"<!-- <react src="./react/Missing.tsx" strategy="visible" /> --><react src="./react/Visible.tsx" strategy="visible" />"#;
        let (out, refs) = transpile_react_tags(
            input,
            &page,
            &root.join("src"),
            &ReactBuildConfig {
                enabled: true,
                dirs: vec!["react".into()],
                ..Default::default()
            },
            &RoutingConfig {
                ignore_directories: vec!["react".into()],
            },
            None,
        )
        .unwrap();

        assert!(out.contains(r#"<!-- <react src="./react/Missing.tsx" strategy="visible" /> -->"#));
        assert_eq!(refs.len(), 1);
        assert!(refs[0].source_path.ends_with("Visible.tsx"));
    }

    #[test]
    fn react_tag_supports_json_props() {
        let root = std::env::temp_dir().join("pilcrow_react_json_props");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/pages/dashboard/react")).unwrap();
        let page = root.join("src/pages/dashboard/index.html");
        fs::write(
            root.join("src/pages/dashboard/react/Grid.tsx"),
            "export default function Grid() { return null }",
        )
        .unwrap();

        let (out, _) = transpile_react_tags(
            r#"<react src="./react/Grid.tsx" strategy="visible" json-rows="{{ rows_json }}" />"#,
            &page,
            &root.join("src"),
            &ReactBuildConfig {
                enabled: true,
                dirs: vec!["react".into()],
                ..Default::default()
            },
            &RoutingConfig {
                ignore_directories: vec!["react".into()],
            },
            Some("/dashboard"),
        )
        .unwrap();

        assert!(out.contains("data-prop-json-rows=\"{{ rows_json }}\""));
    }

    #[test]
    fn react_tag_requires_ignored_directory() {
        let root = std::env::temp_dir().join("pilcrow_react_tag_ignore");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/pages/dashboard/react")).unwrap();
        let page = root.join("src/pages/dashboard/index.html");
        fs::write(
            root.join("src/pages/dashboard/react/Counter.tsx"),
            "export default function Counter() { return null }",
        )
        .unwrap();

        let err = transpile_react_tags(
            r#"<react src="./react/Counter.tsx" strategy="visible" />"#,
            &page,
            &root.join("src"),
            &ReactBuildConfig {
                enabled: true,
                dirs: vec!["react".into()],
                ..Default::default()
            },
            &RoutingConfig::default(),
            None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("ignore_directories"));
    }

    #[test]
    fn shell_strategy_requires_ssr_flag() {
        let root = std::env::temp_dir().join("pilcrow_react_shell_ssr_flag");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/pages/react")).unwrap();
        let page = root.join("src/pages/index.html");
        fs::write(
            root.join("src/pages/react/Button.tsx"),
            "export default function Button() { return null }",
        )
        .unwrap();

        let err = transpile_react_tags(
            r#"<react src="./react/Button.tsx" strategy="shell" />"#,
            &page,
            &root.join("src"),
            &ReactBuildConfig {
                enabled: true,
                ssr: false,
                dirs: vec!["react".into()],
                ..Default::default()
            },
            &RoutingConfig {
                ignore_directories: vec!["react".into()],
            },
            None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("ssr = true"));
    }

    #[test]
    fn shell_strategy_emits_shell_placeholder() {
        let root = std::env::temp_dir().join("pilcrow_react_shell_placeholder");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/pages/react")).unwrap();
        let page = root.join("src/pages/index.html");
        fs::write(
            root.join("src/pages/react/Button.tsx"),
            "export default function Button() { return null }",
        )
        .unwrap();

        let (out, refs) = transpile_react_tags(
            r#"<react src="./react/Button.tsx" strategy="shell" />"#,
            &page,
            &root.join("src"),
            &ReactBuildConfig {
                enabled: true,
                ssr: true,
                dirs: vec!["react".into()],
                ..Default::default()
            },
            &RoutingConfig {
                ignore_directories: vec!["react".into()],
            },
            None,
        )
        .unwrap();

        assert!(out.contains(REACT_SHELL_PLACEHOLDER_PREFIX));
        assert!(out.contains("data-strategy=\"shell\""));
        assert_eq!(refs[0].strategy, "shell");
    }

    #[test]
    fn ssr_strategy_emits_ssr_placeholder() {
        let root = std::env::temp_dir().join("pilcrow_react_ssr_placeholder");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/pages/react")).unwrap();
        let page = root.join("src/pages/index.html");
        fs::write(
            root.join("src/pages/react/Counter.tsx"),
            "export default function Counter() { return null }",
        )
        .unwrap();

        let (out, refs) = transpile_react_tags(
            r#"<react src="./react/Counter.tsx" strategy="ssr" />"#,
            &page,
            &root.join("src"),
            &ReactBuildConfig {
                enabled: true,
                ssr: true,
                dirs: vec!["react".into()],
                ..Default::default()
            },
            &RoutingConfig {
                ignore_directories: vec!["react".into()],
            },
            None,
        )
        .unwrap();

        assert!(out.contains(REACT_SSR_PLACEHOLDER_PREFIX));
        assert!(out.contains("data-strategy=\"ssr\""));
        assert_eq!(refs[0].strategy, "ssr");
    }

    #[test]
    fn react_support_module_exposes_action_api_without_form_dependencies() {
        assert!(PILCROW_REACT_TS.contains("export function submitSilcrow"));
        assert!(PILCROW_REACT_TS.contains("export function silcrowSubmitHandler"));
        assert!(PILCROW_REACT_TS.contains("export function useSilcrowAction"));
        assert!(PILCROW_REACT_TS.contains("export function useSilcrowForm"));
        assert!(PILCROW_REACT_TS.contains("export function useSilcrowResource"));
        assert!(PILCROW_REACT_TS.contains("export function useSilcrowMutation"));
        assert!(PILCROW_REACT_TS.contains("SilcrowMutationOptions"));
        assert!(PILCROW_REACT_TS.contains("SilcrowMutationState"));
        assert!(PILCROW_REACT_TS.contains("publishOptimistic"));
        assert!(PILCROW_REACT_TS.contains("confirmOptimistic"));
        assert!(PILCROW_REACT_TS.contains("revertOptimistic"));
        assert!(PILCROW_REACT_TS.contains("useActionState<State, FormData>"));
        assert!(!PILCROW_REACT_TS.contains("zodFormValidator"));
        assert!(!PILCROW_REACT_TS.contains("safeParse"));
        assert!(!PILCROW_REACT_TS.contains("react-hook-form"));
        assert!(!PILCROW_REACT_TS.contains("from \"zod\""));
    }
}
