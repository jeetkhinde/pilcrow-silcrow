# Atoms and Optimistic UI

Silcrow atoms are reactive client-side stores.

## Scopes

- `route:<pathname>`
- `stream:<url>`
- custom scope names

## Public Store API

`window.Silcrow` exposes:

- `prefetch`
- `submit`
- `subscribe`
- `snapshot`
- `publish`

React islands use these through generated hooks. Plain Silcrow code can subscribe directly.

## Optimistic UI

Optimistic operations snapshot current atom state, apply speculative data, and then confirm or revert when the server responds.

Public methods:

- `publishOptimistic(scope, data, mutationId)`
- `confirmOptimistic(mutationId)`
- `revertOptimistic(mutationId)`

Events:

- `silcrow:optimistic`
- `silcrow:confirmed`
- `silcrow:revert`

Use optimistic UI for interactions that need immediate feedback and can be reconciled by the server.
