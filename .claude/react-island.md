# React Islands — Operating Rules

## DO NOT

- Do NOT scan files to answer React usage questions
- Do NOT open `react.rs`, `react-islands.js`, or `sandbox/react/` for general questions
- Do NOT infer architecture by reading multiple files

## Answer from this model first

React is used ONLY as a view layer. Silcrow owns everything else.

| Silcrow owns | React owns |
|---|---|
| Data fetching | Rendering |
| State synchronization | UI state (forms, local interaction) |
| Network transport | |
| Live updates | |

## Decision guide — use this before touching any file

| Situation | Hook to use |
|---|---|
| Live/shared UI state | `useSilcrowAtom` |
| Read route data (sync) | `useSilcrowRoute` |
| Async initial data + Suspense | `useSilcrowResource` or `useSilcrowPrefetch` |
| Form submission | `useSilcrowForm` or `useSilcrowAction` |
| Named Pilcrow page action | `usePilcrowNamedAction` |
| React Hook Form / complex validation | `silcrowSubmitHandler` |
| Manually patch state | `publishSilcrowAtom` |
| Cross-component sync | `useSilcrowAtom` |

## Core APIs (authoritative — do not re-derive by scanning)

- `useSilcrowAtom(scope, fallback)` — subscribe to live Silcrow atom
- `useSilcrowRoute(path, fallback)` — read route-backed data (alias: `useSilcrowAtom("route:${path}", fallback)`)
- `useSilcrowPrefetch(path)` — memoized Promise for React `use()`
- `useSilcrowResource(path, fallback)` — prefetch + suspend + subscribe in one call; requires `<Suspense>`
- `useSilcrowAction(url, initialState?, options?)` — React 19 `useActionState` backed by Silcrow transport; returns tuple `[state, action, pending]`
- `useSilcrowForm(url, initialState?, options?)` — object wrapper over `useSilcrowAction`; returns `{state, action, pending, ok, message, errors}`
- `usePilcrowNamedAction(name, initialState?, options?)` — resolves a Pilcrow named action relative to the current page
- `submitSilcrow(url, options?)` — raw action function for `useActionState`
- `silcrowSubmitHandler(url, options?)` — async submit callback for React Hook Form
- `publishSilcrowAtom(scope, data)` — manually patch a Silcrow atom
- `resolvePilcrowAction(name, base?)` — resolve a named action to a URL

## Rules

- React 19+ only. `useActionState`, `use()`, and `Suspense` are all available.
- Do not expose HTTP verbs in React hooks. Hooks represent intent (read/subscribe/mutate). Silcrow handles transport.
- Do not write manual `fetch()` for Pilcrow/Silcrow mutations.
- Do not use `useState + useEffect + fetch` for data that Silcrow already owns.

## When to read source files

Only when:
- User asks "how is this implemented?"
- Behavior appears inconsistent with this guide
- Debugging requires exact source

Source of truth: `PILCROW_REACT_TS` constant in `pilcrow/crates/routekit/src/templating/react.rs`
