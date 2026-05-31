use std::fs;
use std::path::{Path, PathBuf};

pub fn handle_new(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("expected: new <dir> [--with-auth] [--with-postgres]".to_string());
    }

    let dir = args.iter().find(|a| !a.starts_with("--")).ok_or_else(|| {
        "expected a directory name as the first positional argument".to_string()
    })?;

    let with_auth = args.iter().any(|a| a == "--with-auth");
    let with_postgres = args.iter().any(|a| a == "--with-postgres");

    let root = PathBuf::from(dir);
    if root.exists() {
        return Err(format!("destination already exists: {}", root.display()));
    }

    create_scaffold(&root, with_auth, with_postgres).map_err(|err| err.to_string())?;

    println!("created Pilcrow app at {}", root.display());
    if with_auth {
        println!("  - auth hook at hooks.rs");
    }
    if with_postgres {
        println!("  - postgres env in .env and Pilcrow.toml");
        println!("  - sqlx added to Cargo.toml");
    }
    println!("\nnext steps:");
    println!("  cd {dir}");
    println!("  cargo run");
    Ok(())
}

fn create_scaffold(root: &Path, with_auth: bool, with_postgres: bool) -> std::io::Result<()> {
    fs::create_dir_all(root.join("src"))?;
    fs::create_dir_all(root.join("pages"))?;
    fs::create_dir_all(root.join("ui"))?;
    fs::create_dir_all(root.join("api"))?;

    let mut cargo_deps = String::from(
        r#"pilcrow-web = { git = "https://github.com/jeetkhinde/Pilcrow" }"#,
    );
    if with_postgres {
        cargo_deps.push_str(
            "\nsqlx = { version = \"0.8\", features = [\"postgres\", \"runtime-tokio\"] }",
        );
    }

    fs::write(
        root.join("Cargo.toml"),
        format!(
            r#"[package]
name = "app"
version = "0.1.0"
edition = "2021"

[dependencies]
{cargo_deps}

[build-dependencies]
pilcrow-routekit = {{ git = "https://github.com/jeetkhinde/Pilcrow" }}
"#
        ),
    )?;

    let mut pilcrow_toml = String::from(
        r#"[web]
host = "127.0.0.1"
port = 3000
"#,
    );
    // Collect all private env vars into one [env.private] block — duplicate tables are a TOML parse error.
    let mut private_env: Vec<(&str, &str)> = Vec::new();
    if with_postgres {
        private_env.push(("DATABASE_URL", "DATABASE_URL"));
    }
    if with_auth {
        private_env.push(("SECRET_KEY", "SECRET_KEY"));
    }
    if !private_env.is_empty() {
        pilcrow_toml.push_str("\n[env.private]\n");
        for (key, env_var) in private_env {
            pilcrow_toml.push_str(&format!("{key} = {{ env = \"{env_var}\" }}\n"));
        }
    }
    fs::write(root.join("Pilcrow.toml"), pilcrow_toml)?;

    fs::write(
        root.join("build.rs"),
        r#"fn main() {
    pilcrow_routekit::compile_current_crate_sources().expect("compile pilcrow html sources");
}
"#,
    )?;

    // main.rs: handles `cargo run -- export <dir>` for static export.
    fs::write(
        root.join("src/main.rs"),
        r#"pilcrow_web::pilcrow_app!();

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("export") {
        let dir = args.next().unwrap_or_else(|| "dist".to_string());
        pilcrow_export(&dir).await;
    } else {
        pilcrow_start().await;
    }
}
"#,
    )?;

    // Root auto-layout.
    fs::write(
        root.join("pages/_layout.html"),
        r#"---
pub struct Props {}
---
<!doctype html>
<html lang="en">
<head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <slot name="title"><title>My App</title></slot>
</head>
<body>
    <slot />
    <script src="{{ pilcrow_web::assets::assets::silcrow_js_path() }}" defer></script>
</body>
</html>
"#,
    )?;

    fs::write(
        root.join("pages/index.html"),
        r#"<Fragment slot="title"><title>Home — My App</title></Fragment>
<main>
    <h1>{{ greeting }}</h1>
    <form s-post="?/greet" s-target="main" id="main" method="post" action="?/greet">
        <label>
            Name
            <input type="text" name="name" />
        </label>
        <button type="submit">Say hello</button>
    </form>
</main>
"#,
    )?;

    fs::write(
        root.join("pages/index.rs"),
        r#"pub struct Props {
    pub greeting: String,
}

pub async fn load(req: Req) -> AppResult<Props> {
    let name = req.query.get("name").unwrap_or("world");
    Ok(Props {
        greeting: format!("Hello, {name}!"),
    })
}

pub async fn greet(req: Req) -> ActionResult {
    let name = req.form.get("name").unwrap_or("world").trim().to_owned();
    redirect(&format!("/?name={name}"))
}
"#,
    )?;

    fs::write(
        root.join("pages/_not_found.html"),
        r#"<Fragment slot="title"><title>404 Not Found — My App</title></Fragment>
<main>
    <h1>404 — Page not found</h1>
    <p><a s-get="/">Go home</a></p>
</main>
"#,
    )?;

    if with_auth {
        fs::write(
            root.join("hooks.rs"),
            r#"use pilcrow_web::{AppError, HookError, Next, Req, Response};
use pilcrow_web::axum::response::IntoResponse;

/// Server startup hook — runs once before the server begins accepting connections.
/// Initialise database pools, caches, and other shared resources here.
pub async fn init() {
    // TODO: initialise your database pool and store it in a global or inject via locals.
}

/// Global request handler — runs for every incoming request.
/// Set values on `req.locals` to share auth context with downstream load() and action fns.
pub async fn handle(req: Req, next: Next) -> Response {
    let token = req.cookies.get("session").map(|c| c.value().to_string());
    match token {
        Some(_token) => {
            // TODO: verify the token and call req.locals.set(your_user_type).
            next.run().await
        }
        None if req.path.starts_with("/dashboard") => {
            AppError::Unauthorized.into_response()
        }
        _ => next.run().await,
    }
}

/// Error hook — called when a route handler returns a 5xx response.
/// Return Some(response) to render a custom error page. Return None for the default.
pub async fn handle_error(error: &HookError, _req: &Req) -> Option<Response> {
    pilcrow_web::tracing::error!(status = error.status, "server error: {}", error.message);
    None
}
"#,
        )?;
    }

    if with_postgres {
        fs::write(
            root.join(".env"),
            "DATABASE_URL=postgres://user:password@localhost/myapp\n",
        )?;
    }

    Ok(())
}
