# React Islands

React is used only as a view layer. Silcrow owns data fetching, transport, state synchronization, and live updates.

## Template Tag

```html
<react src="Counter.tsx" strategy="visible"></react>
```

Rules:

- `src` and `strategy` are required.
- `strategy` must be `load`, `visible`, or `idle`.
- React source must live under one of `[client.react].dirs`.
- React source directories should also be ignored by route discovery.

## Ownership

| Silcrow owns | React owns |
| --- | --- |
| Data fetching | Rendering |
| State synchronization | Local UI state |
| Network transport | Component interaction |
| Live updates | Form controls |

## Hooks

Use the generated React hooks instead of manual `fetch()`:

- `useSilcrowAtom(scope, fallback)`
- `useSilcrowRoute(path, fallback)`
- `useSilcrowPrefetch(path)`
- `useSilcrowResource(path, fallback)`
- `useSilcrowAction(url, initialState, options)`
- `useSilcrowForm(url, initialState, options)`
- `usePilcrowNamedAction(name, initialState, options)`
- `useSilcrowMutation(options)`
- `publishSilcrowAtom(scope, data)`

Do not use `useState + useEffect + fetch` for data Silcrow already owns.
