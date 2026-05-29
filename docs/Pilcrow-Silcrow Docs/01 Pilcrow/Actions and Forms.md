# Actions and Forms

Named actions are server functions in a page or fragment code-behind file. They handle enhanced and plain form posts.

## Action Function

```rust
pub async fn create(req: Req) -> ActionResult {
    let form = req.form.parse::<CreateTodo>()?;
    save_todo(form).await?;
    redirect("/todos")
}
```

Rules:

- Action functions are POST-only.
- Action functions return `ActionResult` or a compatible response result.
- Layouts cannot define named actions.
- UI components cannot define named actions.
- Unknown action names return `NotFound`.

## Form URL

Named action dispatch uses a query-fragment style action name:

```html
<form method="post" action="?/create">
  <input name="title">
  <button>Save</button>
</form>
```

Silcrow can enhance submissions through `s-post`, but plain HTML POST still works.

## Typed Forms

`req.form` and `req.query` can parse into `serde::Deserialize` types:

```rust
#[derive(serde::Deserialize)]
struct CreateTodo {
    title: String,
}

let form = req.form.parse::<CreateTodo>()?;
```

Use the form validator builder when you need accumulated field errors and `req.fail()`.
