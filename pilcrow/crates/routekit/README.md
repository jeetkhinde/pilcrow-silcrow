# pilcrow-routekit

`pilcrow-routekit` is the mandatory web rendering compiler in Pilcrow convention.

## What It Does

- discovers file-based routes from `pages`
- composes `layouts` and `components`
- expands slots and component invocations
- emits generated Rust modules:
  - `generated_routes.rs`
  - `generated_templates.rs`

## Explicit Template Imports (Required)

PascalCase template tags are resolved only from explicit frontmatter imports.

```html
---
import MainLayout from "ui/MainLayout.html";
import StatusBadge from "ui/StatusBadge.html";

pub struct Props {
    pub title: String,
}
---
<MainLayout title={title}>
  <StatusBadge text="ready" />
</MainLayout>
```

- Imports must use a configured alias, like built-in `ui/...`, or a relative `./` / `../` path.
- Import paths must end in `.html` and stay inside the project root.
- Missing imports are compile errors.

## Common Failure (Missing Import)

If a template uses a PascalCase tag without import, routekit fails with an actionable message, for example:

```text
Pilcrow template compile error
  file: pages/index.html
  error: missing explicit import for component `<StatusBadge>` at template line 25, column 6. Add `import StatusBadge from "ui/StatusBadge.html";` in frontmatter.
```

## Required Web Build Integration

```rust
fn main() {
    pilcrow_routekit::compile_current_crate_sources()
        .expect("compile pilcrow html sources");
}
```

No alternate app-level page rendering path is documented.
