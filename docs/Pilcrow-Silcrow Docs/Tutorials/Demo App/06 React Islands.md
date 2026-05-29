# 06 — React Islands

React islands let you mount interactive React components inside server-rendered Pilcrow pages. Everything outside an island is plain HTML — no React, no virtual DOM, no hydration cost.

## Setup

Enable islands in `Pilcrow.toml`:

```toml
[routing]
ignore_directories = ["react"]   # keep routekit from treating react/ as a page dir

[client.react]
enabled       = true
dirs          = ["react"]        # where .tsx source files live
max_island_kb = 80               # per-island JS budget
ssr           = true             # server-render island HTML before hydration
```

Place component source in `react/`:

```text
react/
  DemoCounter.tsx
  Greeting.tsx
```

## Island syntax in templates

```html
<!-- Basic island — no props from the server -->
<react src="/react/DemoCounter.tsx" strategy="visible" />

<!-- Island with server props -->
<react src="/react/Greeting.tsx"
       strategy="shell"
       name="{{ username }}"
       message="Welcome back, member since {{ member_since }}" />
```

Attributes become component props. Minijinja expressions are evaluated server-side before the tag is processed, so `name="{{ username }}"` passes the Rust string to React.

## Strategies

| Strategy | Behaviour |
|---|---|
| `visible` | Hydrates when the element scrolls into viewport. Empty shell until then. |
| `idle` | Hydrates after the browser is idle (`requestIdleCallback`). |
| `load` | Hydrates immediately on page load. |
| `shell` | Server-renders the component HTML, then hydrates. Use for above-the-fold content that must be visible before JS loads. |

## Code-behind for a page with islands

`pages/demo/react-island/index.rs`:

```rust
pub struct Props {
    pub username: String,
    pub member_since: String,
}

pub async fn load(_req: Req) -> AppResult<Props> {
    Ok(Props {
        username: "pilcrow-user".to_string(),
        member_since: "January 2026".to_string(),
    })
}
```

No island-specific Rust code needed. The Rust handler loads whatever data the island needs via Props and passes it as template attributes.

## Island template — `pages/demo/react-island/index.html`

```html
<pilcrow:head>
  <title>React Island Demo — Pilcrow</title>
</pilcrow:head>

<h1>React Islands</h1>

<section>
  <h2>Basic island (no server props)</h2>
  <react src="/react/DemoCounter.tsx" strategy="visible" />
</section>

<section>
  <h2>Island with server props</h2>
  <react src="/react/Greeting.tsx"
         strategy="shell"
         name="{{ username }}"
         message="Welcome back, member since {{ member_since }}" />
</section>
```

## A minimal island component

`react/DemoCounter.tsx`:

```tsx
import { useState } from "react";

export default function DemoCounter() {
  const [count, setCount] = useState(0);
  return (
    <div>
      <p>Count: {count}</p>
      <button onClick={() => setCount(c => c + 1)}>+</button>
    </div>
  );
}
```

`react/Greeting.tsx` with typed server props:

```tsx
interface Props {
  name: string;
  message: string;
}

export default function Greeting({ name, message }: Props) {
  return (
    <div>
      <h3>Hello, {name}!</h3>
      <p>{message}</p>
    </div>
  );
}
```

Props defined as a TypeScript interface match the attribute names in the template. Pilcrow serialises them as JSON and deserialises at hydration.

## Islands on the home page

`pages/index.html` embeds both React and Solid islands:

```html
<react src="/react/Counter.tsx" strategy="visible" initial-count="3" />
<solid src="/solid/Counter.tsx" strategy="visible" initial-count="5" />
```

`initial-count` is passed as a string; the component casts it to a number. Multiple island frameworks can coexist on one page.

## Tutorial complete

You have seen everything the demo app demonstrates:

- ✓ UI component imports with frontmatter
- ✓ External API fetching with `reqwest`
- ✓ FSR multi-counter dashboard (six slots, one query)
- ✓ Object `LiveProp<T>` for compound DOM state (text + class)
- ✓ API routes with `FsrStore` extension
- ✓ React islands with strategies and server props

## Further reading

- [[../../03 Rendering/React Islands]] — full React island reference
- [[../../03 Rendering/Build an FSR Page]] — FSR field reference
- [[../../03 Rendering/FSR SSE Hub]] — scalar vs object rule, class trap
- [[../../01 Pilcrow/API Routes and Fragments]] — raw Axum API routes
- [[../../05 Reference/FSR Ownership and Invalidation]] — invalidation patterns
