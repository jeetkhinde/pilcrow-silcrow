# Build Your First Page

This guide teaches the basic Pilcrow page shape before FSR, islands, or React.

## What You Will Build

A `/todos` page that:

- Loads rows on GET.
- Renders them in HTML.
- Handles a form POST through a named action.
- Redirects after success.

## File Layout

```text
pages/
  todos/
    index.html
    index.rs
```

## Step 1: Create `index.rs`

```rust
use pilcrow_web::{redirect, ActionResult, AppError, AppResult, Req};

#[derive(sqlx::FromRow)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub done: bool,
}

pub struct Props {
    pub todos: Vec<Todo>,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    let pool = crate::db::pool();

    let todos = sqlx::query_as::<_, Todo>(
        "SELECT id, title, done FROM todos ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::Internal)?;

    Ok(Props { todos })
}
```

Rules:

- `Props` is the template data.
- `load()` runs for GET requests.
- `load()` must be async.
- `load()` accepts `Req`.
- `load()` returns `AppResult<Props>`.

## Step 2: Render `index.html`

```html
<h1>Todos</h1>

<ul>
  {% for todo in todos %}
    <li>
      {% if todo.done %}<s>{{ todo.title }}</s>{% else %}{{ todo.title }}{% endif %}
    </li>
  {% endfor %}
</ul>
```

Pilcrow compiles this template through routekit and Askama. You do not manually register an Axum route.

## Step 3: Add a Form

```html
<form method="post" action="?/create">
  <input name="title" required>
  <button type="submit">Add</button>
</form>
```

The action name is `create`, so Pilcrow dispatches the POST to a function named `create` in `index.rs`.

## Step 4: Add the Named Action

```rust
#[derive(serde::Deserialize)]
struct CreateTodo {
    title: String,
}

pub async fn create(req: Req) -> ActionResult {
    let form = req.form.parse::<CreateTodo>()?;
    let pool = crate::db::pool();

    sqlx::query("INSERT INTO todos (title, done) VALUES ($1, false)")
        .bind(form.title)
        .execute(pool)
        .await
        .map_err(|_| AppError::Internal)?;

    redirect("/todos")
}
```

Rules:

- Named actions are POST-only.
- The function name matches the form action suffix.
- Use `req.form.parse::<T>()` for typed form data.
- Return `redirect()` after successful mutation.

## Step 5: Add a Layout

Create:

```text
pages/_layout.html
```

```html
<!doctype html>
<html>
  <head>
    <title>Pilcrow App</title>
  </head>
  <body>
    <main>
      {{ content|safe }}
    </main>
  </body>
</html>
```

All child pages render inside `{{ content|safe }}`.

## Step 6: Know What Pilcrow Generated

At build time, routekit discovers:

- `pages/todos/index.html`
- `pages/todos/index.rs`
- `pages/_layout.html`

Then it emits generated modules in `OUT_DIR` and wires the route into the generated Axum router.

The application code should call the generated app entry point through `pilcrow_app!()`. Do not hand-register this route in `main.rs`.

## Next Step

After this page works, add FSR by following [[../03 Rendering/Build an FSR Page]].
