use crate::workspace::{display_path, ensure_contained, resolve_project};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
pub struct ScaffoldFile {
    pub path: String,
    pub action: ScaffoldAction,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaffoldAction {
    Create,
    Update,
    SkipExists,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScaffoldResult {
    pub dry_run: bool,
    pub files: Vec<ScaffoldFile>,
    pub validation_notes: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct ScaffoldRequest<'a> {
    pub kind: &'a str,
    pub name: &'a str,
    pub route_path: Option<&'a str>,
    pub target_dir: Option<&'a str>,
    pub options: Option<&'a Value>,
    pub dry_run: bool,
    pub overwrite: bool,
}

pub fn orchestrate_feature(
    current_root: &Path,
    project_root: Option<&str>,
    manifest_path: Option<&str>,
    request: ScaffoldRequest<'_>,
) -> Result<ScaffoldResult> {
    let resolved = resolve_project(current_root, project_root, manifest_path)?;
    let (mut files, validation_notes) = match request.kind {
        "route" | "page" | "loaded-page" => scaffold_loaded_page(&resolved.app_root, request)?,
        "static-page" => scaffold_static_page(&resolved.app_root, request)?,
        "action-page" => scaffold_action_page(&resolved.app_root, request)?,
        "deferred-page" => scaffold_deferred_page(&resolved.app_root, request)?,
        "component" => scaffold_component(&resolved.app_root, request)?,
        "fragment" => scaffold_fragment(&resolved.app_root, request)?,
        "silcrow-form" | "silcrow" => scaffold_silcrow_form(&resolved.app_root, request)?,
        "nested-layout" => scaffold_nested_layout(&resolved.app_root, request)?,
        "loading-page" => scaffold_loading_page(&resolved.app_root, request)?,
        "error-page" => scaffold_error_page(&resolved.app_root, request)?,
        "not-found-page" => scaffold_not_found_page(&resolved.app_root, request)?,
        "api-route" | "api" => scaffold_api_route(&resolved.app_root, request)?,
        "middleware" => scaffold_middleware(&resolved.app_root, request)?,
        "env-config" => scaffold_env_config(&resolved.app_root, request)?,
        "typed-param" => scaffold_typed_param(&resolved.app_root, request)?,
        other => bail!("unsupported scaffold kind: {other}. Supported: route, static-page, loaded-page, action-page, deferred-page, component, fragment, silcrow-form, nested-layout, loading-page, error-page, not-found-page, api-route, middleware, env-config, typed-param"),
    };

    for file in &mut files {
        let path = PathBuf::from(&file.path);
        ensure_contained(&resolved.app_root, &path)?;
        if path.exists() && !request.overwrite && !matches!(file.action, ScaffoldAction::Update) {
            file.action = ScaffoldAction::SkipExists;
            continue;
        }
        if !request.dry_run && !matches!(file.action, ScaffoldAction::SkipExists) {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, &file.content)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
    }

    Ok(ScaffoldResult {
        dry_run: request.dry_run,
        files,
        validation_notes,
    })
}

// --- static page: no code-behind ---
fn scaffold_static_page(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let route_path = request.route_path.unwrap_or(request.name);
    let html_path = route_file(app_root, route_path, "html");
    let title = titleize(request.name);
    Ok((
        vec![ScaffoldFile {
            path: display_path(&html_path),
            action: ScaffoldAction::Create,
            content: format!(
                "<Fragment slot=\"title\"><title>{title}</title></Fragment>\n\n<h1>{title}</h1>\n<p>Static page — no server data needed.</p>\n"
            ),
        }],
        vec!["Static page: no code-behind. Add a .rs file with load() if you need server data.".to_string()],
    ))
}

// --- loaded page: with Props and load() ---
fn scaffold_loaded_page(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let route_path = request.route_path.unwrap_or(request.name);
    let html_path = route_file(app_root, route_path, "html");
    let rs_path = route_file(app_root, route_path, "rs");
    let title = titleize(request.name);
    let with_action = option_bool(request.options, "action").unwrap_or(false);
    let is_dynamic = route_path.contains('[') && route_path.contains(']');

    let mut rs_content = if is_dynamic {
        format!(
            "pub struct Props {{\n    pub title: &'static str,\n}}\n\npub async fn load(ctx: Page) -> AppResult<Props> {{\n    Ok(Props {{ title: \"{title}\" }})\n}}\n"
        )
    } else {
        format!(
            "pub struct Props {{\n    pub title: &'static str,\n}}\n\npub async fn load(_req: Req) -> AppResult<Props> {{\n    Ok(Props {{ title: \"{title}\" }})\n}}\n"
        )
    };
    if with_action {
        rs_content.push_str(
            "\npub async fn submit(req: Req) -> ActionResult {\n    req.res.no_cache();\n    redirect(&req.path)\n}\n",
        );
    }

    Ok((
        vec![
            ScaffoldFile {
                path: display_path(&html_path),
                action: ScaffoldAction::Create,
                content: format!(
                    "<Fragment slot=\"title\"><title>{{{{ title }}}}</title></Fragment>\n\n<h1>{{{{ title }}}}</h1>\n"
                ),
            },
            ScaffoldFile {
                path: display_path(&rs_path),
                action: ScaffoldAction::Create,
                content: rs_content,
            },
        ],
        vec![if is_dynamic {
            "Loaded dynamic page: Props and load(ctx: Page) are scaffolded. Route params are generated from the file path as `Params`.".to_string()
        } else {
            "Loaded page: Props and load() are scaffolded. Extend Props with the data your page needs.".to_string()
        }],
    ))
}

// --- action page: loaded + named action ---
fn scaffold_action_page(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let route_path = request.route_path.unwrap_or(request.name);
    let html_path = route_file(app_root, route_path, "html");
    let rs_path = route_file(app_root, route_path, "rs");
    let title = titleize(request.name);
    let action_name = option_str(request.options, "action_name").unwrap_or("submit");

    Ok((
        vec![
            ScaffoldFile {
                path: display_path(&html_path),
                action: ScaffoldAction::Create,
                content: format!(
                    "<Fragment slot=\"title\"><title>{title}</title></Fragment>\n\n<h1>{title}</h1>\n<form method=\"post\" action=\"?/{action_name}\">\n    <button type=\"submit\">Submit</button>\n</form>\n"
                ),
            },
            ScaffoldFile {
                path: display_path(&rs_path),
                action: ScaffoldAction::Create,
                content: format!(
                    "pub struct Props {{\n    pub title: &'static str,\n}}\n\npub async fn load(_req: Req) -> AppResult<Props> {{\n    Ok(Props {{ title: \"{title}\" }})\n}}\n\npub async fn {action_name}(req: Req) -> ActionResult {{\n    // Handle the form post\n    redirect(&req.path)\n}}\n"
                ),
            },
        ],
        vec![format!("Action page: named action '{}' is scaffolded. It is invoked by POST ?/{}.", action_name, action_name)],
    ))
}

// --- deferred page: with AsyncValue<T> field ---
fn scaffold_deferred_page(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let route_path = request.route_path.unwrap_or(request.name);
    let html_path = route_file(app_root, route_path, "html");
    let rs_path = route_file(app_root, route_path, "rs");
    let title = titleize(request.name);

    Ok((
        vec![
            ScaffoldFile {
                path: display_path(&html_path),
                action: ScaffoldAction::Create,
                content: format!(
                    "<Fragment slot=\"title\"><title>{title}</title></Fragment>\n\n<h1>{title}</h1>\n<p>Count: <span :text=\"count\">Loading...</span></p>\n"
                ),
            },
            ScaffoldFile {
                path: display_path(&rs_path),
                action: ScaffoldAction::Create,
                content: format!(
                    "use pilcrow_web::AsyncValue;\n\npub struct Props {{\n    pub title: &'static str,\n    pub count: AsyncValue<i32>,\n}}\n\npub async fn load(_req: Req) -> AppResult<Props> {{\n    Ok(Props {{\n        title: \"{title}\",\n        count: AsyncValue::spawn(async {{\n            // Replace with your expensive async computation\n            42\n        }}),\n    }})\n}}\n"
                ),
            },
        ],
        vec![
            "AsyncValue page: the shell renders immediately; 'count' is streamed after it resolves.".to_string(),
            "Add a _loading.html template in this route's directory for a loading skeleton.".to_string(),
            "AsyncValue<T> requires T: Display for the initial empty render.".to_string(),
        ],
    ))
}

// --- nested layout ---
fn scaffold_nested_layout(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let route_path = request.route_path.unwrap_or(request.name);
    let dir = app_root
        .join("pages")
        .join(route_path.trim_start_matches('/').trim_end_matches('/'));
    let html_path = dir.join("_layout.html");

    let with_code = option_bool(request.options, "code_behind").unwrap_or(false);
    let mut files = vec![ScaffoldFile {
        path: display_path(&html_path),
        action: ScaffoldAction::Create,
        content: "<nav><!-- nested layout navigation --></nav>\n{{ content }}\n".to_string(),
    }];

    if with_code {
        let rs_path = dir.join("_layout.rs");
        files.push(ScaffoldFile {
            path: display_path(&rs_path),
            action: ScaffoldAction::Create,
            content: "pub struct Props {\n    pub section: &'static str,\n}\n\npub async fn load(_req: Req) -> AppResult<Props> {\n    Ok(Props { section: \"section\" })\n}\n".to_string(),
        });
    }

    Ok((
        files,
        vec![
            "Nested layout: applies to all pages in its directory subtree.".to_string(),
            "Layouts cannot define named actions — only load() is permitted.".to_string(),
        ],
    ))
}

// --- loading page (_loading.html) ---
fn scaffold_loading_page(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let route_path = request.route_path.unwrap_or("");
    let dir = if route_path.is_empty() || route_path == "/" {
        app_root.join("pages")
    } else {
        app_root
            .join("pages")
            .join(route_path.trim_start_matches('/').trim_end_matches('/'))
    };
    let path = dir.join("_loading.html");

    Ok((
        vec![ScaffoldFile {
            path: display_path(&path),
            action: ScaffoldAction::Create,
            content: format!(
                "<!-- Loading skeleton for {} routes -->\n<div class=\"skeleton\">\n    <div class=\"skeleton-line\"></div>\n    <div class=\"skeleton-line short\"></div>\n</div>\n",
                if route_path.is_empty() { "all" } else { route_path }
            ),
        }],
        vec![
            "_loading.html is injected as a <template> element — not a rendered route.".to_string(),
            "Silcrow shows this skeleton during navigation to any route in this directory.".to_string(),
        ],
    ))
}

// --- error page (_error.html) ---
fn scaffold_error_page(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let route_path = request.route_path.unwrap_or("");
    let dir = if route_path.is_empty() || route_path == "/" {
        app_root.join("pages")
    } else {
        app_root
            .join("pages")
            .join(route_path.trim_start_matches('/').trim_end_matches('/'))
    };
    let path = dir.join("_error.html");

    Ok((
        vec![ScaffoldFile {
            path: display_path(&path),
            action: ScaffoldAction::Create,
            content: "<!-- Error page: shown when load() returns Err -->\n<h1>Something went wrong</h1>\n<p>{{ error }}</p>\n".to_string(),
        }],
        vec![
            "_error.html is shown when load() returns an Err (AppError). It is not a route.".to_string(),
        ],
    ))
}

// --- not-found page (_not_found.html) ---
fn scaffold_not_found_page(
    app_root: &Path,
    _request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let path = app_root.join("pages/_not_found.html");

    Ok((
        vec![ScaffoldFile {
            path: display_path(&path),
            action: ScaffoldAction::Create,
            content: "<!-- Not-found page: axum fallback for unmatched routes -->\n<h1>404 — Page Not Found</h1>\n<p><a href=\"/\">Return home</a></p>\n".to_string(),
        }],
        vec![
            "_not_found.html is the axum fallback handler — shown for all unmatched routes.".to_string(),
            "Only one _not_found.html is used (root level).".to_string(),
        ],
    ))
}

// --- api route ---
fn scaffold_api_route(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let filename = format!("{}.rs", kebab_name(request.name));
    let path = app_root.join("api").join(&filename);
    let fn_name = snake_name(request.name);

    Ok((
        vec![ScaffoldFile {
            path: display_path(&path),
            action: ScaffoldAction::Create,
            content: format!(
                "use axum::{{routing::get, Router}};\nuse pilcrow_web::{{json, AppResult}};\nuse serde::Serialize;\n\n#[derive(Serialize)]\nstruct {name}Response {{\n    ok: bool,\n}}\n\nasync fn {fn_name}() -> AppResult<axum::response::Response> {{\n    Ok(json({name}Response {{ ok: true }}))\n}}\n\npub fn router() -> Router {{\n    Router::new().route(\"/api/{route}\", get({fn_name}))\n}}\n",
                name = pascal_name(request.name),
                fn_name = fn_name,
                route = kebab_name(request.name),
            ),
        }],
        vec![
            "API route: exports router() which is auto-mounted by the build pipeline.".to_string(),
            "Do not mount this router manually in main.rs.".to_string(),
        ],
    ))
}

// --- middleware ---
fn scaffold_middleware(
    app_root: &Path,
    _request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let path = app_root.join("hooks.rs");

    Ok((
        vec![ScaffoldFile {
            path: display_path(&path),
            action: ScaffoldAction::Create,
            content: "use pilcrow_web::{AppError, Next, Req, Response};\nuse axum::response::IntoResponse;\n\npub async fn middleware(req: Req, next: Next) -> Response {\n    // Example: auth check for /admin routes\n    // let token = req.cookies.get(\"session\").map(|c| c.value().to_string());\n    // if req.path.starts_with(\"/admin\") && token.is_none() {\n    //     return AppError::Unauthorized.into_response();\n    // }\n    next.run().await\n}\n".to_string(),
        }],
        vec![
            "Middleware must be named `middleware` and live at hooks.rs.".to_string(),
            "The build pipeline auto-detects this file and wraps the router.".to_string(),
            "req.locals and req.res set here are shared with all load() and action fns.".to_string(),
        ],
    ))
}

// --- env config ---
fn scaffold_env_config(
    app_root: &Path,
    _request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let pilcrow_path = app_root.join("Pilcrow.toml");
    let source = fs::read_to_string(&pilcrow_path).unwrap_or_default();

    let has_env = source.contains("[env]");
    if has_env {
        return Ok((
            vec![],
            vec![
                "Pilcrow.toml already has an [env] section. Edit it directly to add vars."
                    .to_string(),
            ],
        ));
    }

    let updated = if source.trim().is_empty() {
        "[env]\npublic = [\"PUBLIC_API_URL\"]\nprivate = [\"DATABASE_URL\"]\n".to_string()
    } else {
        format!(
            "{}\n\n[env]\npublic = [\"PUBLIC_API_URL\"]\nprivate = [\"DATABASE_URL\"]\n",
            source.trim_end()
        )
    };

    Ok((
        vec![ScaffoldFile {
            path: display_path(&pilcrow_path),
            action: ScaffoldAction::Update,
            content: updated,
        }],
        vec![
            "Env config: add your actual env var names to Pilcrow.toml [env].".to_string(),
            "Public vars get PUBLIC_ prefix convention for client-visible use.".to_string(),
            "Generated env::Public::load() and env::Private::load() are in OUT_DIR after build."
                .to_string(),
        ],
    ))
}

// --- typed param matcher ---
fn scaffold_typed_param(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let filename = format!("{}.rs", kebab_name(request.name));
    let path = app_root.join("params").join(&filename);
    let param_name = kebab_name(request.name);

    let validation_body = match param_name.as_str() {
        "integer" | "int" => "    value.parse::<i64>().is_ok()".to_string(),
        "uuid" => "    // Basic UUID format check\n    value.len() == 36 && value.chars().filter(|c| *c == '-').count() == 4".to_string(),
        "slug" => "    !value.is_empty() && value.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')".to_string(),
        _ => "    // Add your validation logic here\n    !value.is_empty()".to_string(),
    };

    Ok((
        vec![ScaffoldFile {
            path: display_path(&path),
            action: ScaffoldAction::Create,
            content: format!(
                "pub fn match_param(value: &str) -> bool {{\n{validation_body}\n}}\n\n#[cfg(test)]\nmod tests {{\n    use super::*;\n\n    #[test]\n    fn rejects_empty() {{\n        assert!(!match_param(\"\"));\n    }}\n}}\n"
            ),
        }],
        vec![
            format!("Typed param: use [id={}] in a route directory name to apply this matcher.", kebab_name(request.name)),
            "match_param() is called at request time — return false to yield 404.".to_string(),
        ],
    ))
}

// --- silcrow form (original) ---
fn scaffold_silcrow_form(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let route_path = request.route_path.unwrap_or(request.name);
    let html_path = route_file(app_root, route_path, "html");
    let rs_path = route_file(app_root, route_path, "rs");
    let title = titleize(request.name);
    Ok((
        vec![
            ScaffoldFile {
                path: display_path(&html_path),
                action: ScaffoldAction::Create,
                content: format!(
                    "<Fragment slot=\"title\"><title>{title}</title></Fragment>\n\n<h1>{title}</h1>\n<form method=\"post\" action=\"?/submit\" s-target=\"#form-result\">\n    <label>\n        Name\n        <input name=\"name\" value=\"{{{{ name }}}}\" />\n    </label>\n    <button type=\"submit\">Submit</button>\n</form>\n<div id=\"form-result\">{{{{ message }}}}</div>\n"
                ),
            },
            ScaffoldFile {
                path: display_path(&rs_path),
                action: ScaffoldAction::Create,
                content: "pub struct Props {\n    pub name: String,\n    pub message: String,\n}\n\npub async fn load(mut req: Req) -> AppResult<Props> {\n    let flash = req.take_form_flash();\n    Ok(Props {\n        name: flash.as_ref().and_then(|f| f.value(\"name\")).unwrap_or_default().to_string(),\n        message: String::new(),\n    })\n}\n\npub async fn submit(req: Req) -> ActionResult {\n    let name = req.form.get(\"name\").unwrap_or(\"\").to_string();\n    if name.is_empty() {\n        return req.fail(form_errors().error(\"name\", \"Name is required\").value(\"name\", \"\"));\n    }\n    req.res.patch_target(\"#form-result\", &format!(\"Thanks, {}\", name));\n    redirect(&req.path)\n}\n".to_string(),
            },
        ],
        vec![
            "Silcrow form: uses s-target for enhanced patching and plain POST fallback.".to_string(),
            "req.fail() returns JSON for enhanced requests, flash+redirect for plain POST.".to_string(),
            "Use silcrow-mcp for exact client-side directive semantics.".to_string(),
        ],
    ))
}

// --- component ---
fn scaffold_component(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let dir = request
        .target_dir
        .map(|dir| app_root.join(dir))
        .unwrap_or_else(|| app_root.join("ui"));
    let path = dir.join(format!("{}.html", pascal_name(request.name)));
    Ok((
        vec![ScaffoldFile {
            path: display_path(&path),
            action: ScaffoldAction::Create,
            content: format!(
                "<section class=\"{}\">\n    {{{{ slot }}}}\n</section>\n",
                kebab_name(request.name)
            ),
        }],
        vec![
            "Component: import in templates with `{% import \"ui/ComponentName.html\" as Comp %}`."
                .to_string(),
        ],
    ))
}

// --- fragment ---
fn scaffold_fragment(
    app_root: &Path,
    request: ScaffoldRequest<'_>,
) -> Result<(Vec<ScaffoldFile>, Vec<String>)> {
    let pilcrow_path = app_root.join("Pilcrow.toml");
    let source = fs::read_to_string(&pilcrow_path).unwrap_or_default();
    let has_fragment_config = source.contains("[[fragments]]");
    let fragment_dir = configured_fragment_dir(&source).unwrap_or_else(|| "widgets".to_string());
    let target_dir = request
        .target_dir
        .map(|dir| app_root.join(dir))
        .unwrap_or_else(|| app_root.join(&fragment_dir));
    let path = target_dir.join(format!("{}.html", kebab_name(request.name)));
    let mut files = vec![ScaffoldFile {
        path: display_path(&path),
        action: ScaffoldAction::Create,
        content: format!(
            "<article id=\"{}\">\n    {{{{ slot }}}}\n</article>\n",
            kebab_name(request.name)
        ),
    }];
    if !has_fragment_config {
        let updated = if source.trim().is_empty() {
            "[[fragments]]\ndir = \"widgets\"\n".to_string()
        } else {
            format!(
                "{}\n\n[[fragments]]\ndir = \"widgets\"\n",
                source.trim_end()
            )
        };
        files.push(ScaffoldFile {
            path: display_path(&pilcrow_path),
            action: ScaffoldAction::Update,
            content: updated,
        });
    }
    Ok((
        files,
        vec![
            "Fragment: accessible as GET /widgets/name with no layout wrapping.".to_string(),
            "Module auto-available as fragments::widgets::name in code-behind files.".to_string(),
        ],
    ))
}

fn route_file(app_root: &Path, route_path: &str, ext: &str) -> PathBuf {
    let cleaned = route_path
        .trim()
        .trim_start_matches('/')
        .trim_end_matches('/');
    if cleaned.is_empty() {
        app_root.join("pages").join(format!("index.{ext}"))
    } else {
        app_root
            .join("pages")
            .join(cleaned)
            .join(format!("index.{ext}"))
    }
}

fn option_bool(options: Option<&Value>, key: &str) -> Option<bool> {
    options
        .and_then(|options| options.get(key))
        .and_then(Value::as_bool)
}

fn option_str<'a>(options: Option<&'a Value>, key: &str) -> Option<&'a str> {
    options
        .and_then(|options| options.get(key))
        .and_then(Value::as_str)
}

fn configured_fragment_dir(source: &str) -> Option<String> {
    let value = toml::from_str::<toml::Value>(source).ok()?;
    let fragments = value.get("fragments")?.as_array()?;
    fragments
        .first()?
        .get("dir")?
        .as_str()
        .map(|dir| dir.trim_start_matches('/').to_string())
}

fn titleize(input: &str) -> String {
    input
        .split(['-', '_', '/', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn pascal_name(input: &str) -> String {
    let title = titleize(input);
    title.replace(' ', "")
}

fn kebab_name(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn snake_name(input: &str) -> String {
    kebab_name(input).replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_scaffold_defaults_to_dry_run_files() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("app")).unwrap();
        fs::write(
            temp.path().join("app/Cargo.toml"),
            "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        )
        .unwrap();
        let result = orchestrate_feature(
            temp.path(),
            Some(temp.path().to_str().unwrap()),
            Some("app/Cargo.toml"),
            ScaffoldRequest {
                kind: "loaded-page",
                name: "reports",
                route_path: Some("/reports"),
                target_dir: None,
                options: None,
                dry_run: true,
                overwrite: false,
            },
        )
        .unwrap();
        assert_eq!(result.files.len(), 2);
        assert!(result.files[0].path.ends_with("pages/reports/index.html"));
    }

    #[test]
    fn existing_file_is_skipped_without_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let app = temp.path().join("app");
        fs::create_dir_all(app.join("pages/reports")).unwrap();
        fs::write(
            app.join("Cargo.toml"),
            "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        )
        .unwrap();
        fs::write(app.join("pages/reports/index.html"), "old").unwrap();
        let result = orchestrate_feature(
            temp.path(),
            Some(temp.path().to_str().unwrap()),
            Some("app/Cargo.toml"),
            ScaffoldRequest {
                kind: "loaded-page",
                name: "reports",
                route_path: Some("/reports"),
                target_dir: None,
                options: None,
                dry_run: false,
                overwrite: false,
            },
        )
        .unwrap();
        assert!(matches!(result.files[0].action, ScaffoldAction::SkipExists));
        assert_eq!(
            fs::read_to_string(app.join("pages/reports/index.html")).unwrap(),
            "old"
        );
    }

    #[test]
    fn api_route_scaffold_creates_router_fn() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("app")).unwrap();
        fs::write(
            temp.path().join("app/Cargo.toml"),
            "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        )
        .unwrap();
        let result = orchestrate_feature(
            temp.path(),
            Some(temp.path().to_str().unwrap()),
            Some("app/Cargo.toml"),
            ScaffoldRequest {
                kind: "api-route",
                name: "users",
                route_path: None,
                target_dir: None,
                options: None,
                dry_run: true,
                overwrite: false,
            },
        )
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].content.contains("pub fn router()"));
        assert!(result.files[0].path.ends_with("api/users.rs"));
    }

    #[test]
    fn middleware_scaffold_creates_middleware_fn() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("app")).unwrap();
        fs::write(
            temp.path().join("app/Cargo.toml"),
            "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        )
        .unwrap();
        let result = orchestrate_feature(
            temp.path(),
            Some(temp.path().to_str().unwrap()),
            Some("app/Cargo.toml"),
            ScaffoldRequest {
                kind: "middleware",
                name: "middleware",
                route_path: None,
                target_dir: None,
                options: None,
                dry_run: true,
                overwrite: false,
            },
        )
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].content.contains("pub async fn middleware"));
        assert!(result.files[0].path.ends_with("hooks.rs"));
    }

    #[test]
    fn typed_param_scaffold_creates_match_param_fn() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("app")).unwrap();
        fs::write(
            temp.path().join("app/Cargo.toml"),
            "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        )
        .unwrap();
        let result = orchestrate_feature(
            temp.path(),
            Some(temp.path().to_str().unwrap()),
            Some("app/Cargo.toml"),
            ScaffoldRequest {
                kind: "typed-param",
                name: "integer",
                route_path: None,
                target_dir: None,
                options: None,
                dry_run: true,
                overwrite: false,
            },
        )
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(result.files[0].content.contains("pub fn match_param"));
        assert!(result.files[0].path.ends_with("params/integer.rs"));
    }
}
