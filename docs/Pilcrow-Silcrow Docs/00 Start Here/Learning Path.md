# Learning Path

Use this path when learning Pilcrow from scratch.

## 1. Build a Normal SSR Page

Read:

- [[../01 Pilcrow/Build Your First Page]]
- [[../01 Pilcrow/Mental Model]]
- [[../01 Pilcrow/Routing]]
- [[../01 Pilcrow/Pages and Layouts]]

Goal:

- Create `pages/index.html`.
- Add `pages/index.rs`.
- Define `Props`.
- Implement `load(req: Req) -> AppResult<Props>`.
- Render props in the template.

## 2. Add a Layout

Read:

- [[../01 Pilcrow/Pages and Layouts]]

Goal:

- Add `pages/_layout.html`.
- Put `{{ content|safe }}` where page content should render.
- Optionally add `_layout.rs` for shared request data.

## 3. Add a Form Action

Read:

- [[../01 Pilcrow/Actions and Forms]]

Goal:

- Add a named action in page code-behind.
- Submit a plain HTML form to `?/action_name`.
- Parse `req.form`.
- Return `redirect()` or `req.fail()`.

## 4. Add FSR and Live Fields

Read:

- [[../03 Rendering/Build an FSR Page]]
- [[../03 Rendering/Live Props and FSR]]
- [[../05 Reference/FSR Ownership and Invalidation]]

Goal:

- Define inline `pub struct Live` in the page code-behind.
- Add `LiveProp<T>` fields.
- Implement `Live::query()`.
- Add `s-live="field_name"` to the HTML elements that should patch.
- Pass `live: Live` into `load()`.
- Call `req.fsr.invalidate_route(path)` from actions that change live data.
- Understand the ownership boundary: `load()` fetches data only; the framework manages slot registration, hit counting, and watcher re-execution.

## 5. Build a Real App End to End

Read:

- [[../01 Pilcrow/Address Book Tutorial]]

Goal:

- See all concepts wired together: route groups, layout loading, multiple routes with different FSR shapes, named actions, Silcrow navigation, PS fragment navigation, and client-side active-state sync.
- Understand when to use `PROMOTE_AFTER` (static routes only) and when to omit it (dynamic routes).
- Understand the `silcrow:navigate` / `silcrow:load` SSE reconnect split.

## 6. Add Revalidation and Debounce

Read:

- [[../03 Rendering/Build an FSR Page]]
- [[../FSR Rendering/Revalidation mechanism]]
- [[../Debounce]]

Goal:

- Use `#[revalidate(N)]` on `Props` `LiveProp<T>` fields for timer-driven invalidation.
- Use `[fsr] revalidate_seconds` for a global fallback.
- Use `#[debounce(N)]` on inline `Live` only when you understand the current runtime caveat.

## 7. Add Islands or React

Read:

- [[../03 Rendering/Islands]]
- [[../03 Rendering/React Islands]]
- [[../02 Silcrow/Silcrow Runtime]]

Goal:

- Choose server-rendered islands for separately loaded HTML.
- Choose React islands only when the component needs React as a view layer.

## 8. Understand Silcrow Navigation

Read:

- [[../02 Silcrow/Navigation and Live Connections]]
- [[../03 Rendering/FSR SSE Hub]]

Goal:

- Know the difference between `silcrow:navigate` (fires before fetch and DOM swap) and `silcrow:load` (fires after DOM swap and `pushState`).
- Know when PS fragment navigation applies and when it falls back to full navigation.
- Know how to keep client-only state (e.g. sidebar active highlight) in sync across fragment navigations using `silcrow:load`.
