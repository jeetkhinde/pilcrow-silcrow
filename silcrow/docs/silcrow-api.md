## HTML Attributes (Directives)

**Bindings**
- `s-use`
- `:text`
- `:class`
- `:style`
- `:show`
- `:value`
- `:checked`
- `:disabled`
- `:selected`
- `:hidden`
- `:required`
- `:readOnly`
- `:src`
- `:href`
- `:selectedIndex`
- `:key`

**Loops**
- `s-for`

**Verbs (Navigation)**
- `s-get`
- `s-post`
- `s-put`
- `s-delete`
- `s-patch`

**Navigation Modifiers**
- `s-target`
- `s-timeout`
- `s-html`
- `s-skip-history`
- `s-preload`
- `no-boost` — opt out of global boost interception for a specific `<a>` element

**Layout-Aware Navigation (auto-injected by Pilcrow codegen)**
- `data-ps-layout="<id>"` — marks the boundary of each auto-layout level (e.g. `"/"`, `"/tickets"`); read by Silcrow to build `X-PS-Present` header
- `data-ps-slot="<pattern>"` — marks the page-content insertion point within a layout (e.g. `"/tickets/:id"`); Silcrow swaps only this element on fragment navigations

**Live Connections**
- `s-sse`
- `s-ws`
- `s-wss`
- `data-pilcrow-live="<url>"` — auto-injected by Pilcrow codegen for pages with `LiveProp<T>` fields; Silcrow discovers this element and opens a managed SSE connection to `url`. Do not write manually.
- `data-pilcrow-list="<field>"` — marks a list container; `list-patch` events target rows within this element by `data-pilcrow-key`. Auto-injected by Pilcrow codegen (Slice F).
- `data-pilcrow-key="<key>"` — marks a list row with its unique key; targeted by `list-patch` events. Auto-injected by Pilcrow codegen (Slice F).

**Atoms / Store**
- `s-bind`

**Debug**
- `s-debug`

---

## Request Headers (Client → Server)

- `silcrow-target` — marks enhanced requests
- `silcrow-mutation-id` — identifies an in-flight optimistic mutation
- `X-PS-Present` — comma-separated list of `data-ps-layout` values currently in the DOM (e.g. `"/,/tickets"`); server uses this to determine whether a fragment-only response is safe

## Response Headers (Server → Client)

- `silcrow-patch` (response header — JSON array of `{target, data[, mutation_id]}` entries)
- `silcrow-invalidate`
- `silcrow-navigate`
- `silcrow-sse`
- `silcrow-ws`
- `silcrow-trigger`
- `silcrow-retarget`
- `silcrow-push`
- `silcrow-cache`
- `silcrow-full-reload`
- `Content-Type: text/html; x-ps-fragment=1` — signals a PS layout fragment response; Silcrow swaps `[data-ps-slot]` in the DOM and upserts head elements from `<template data-ps-head>`

---

## SSE Event Types

- `message` (default)
- `patch` — payload: `{ target, data[, mutation_id] }`. When `mutation_id` is present, silcrow.js calls `confirmOptimistic(mutation_id)` before applying the patch.
- `html`
- `invalidate`
- `navigate`
- `custom`
- `live` — payload: flat JSON object `{ "<field>": <value>, ... }`. Patches all `[data-pilcrow-live-field="field"]` text nodes. Emitted by Pilcrow's per-page SSE route (`/__pilcrow/live{pattern}`) for `LiveProp<T>` fields.
- `list-patch` — payload: `{ "list": "<field>", "key": "<row_key>", "<changed_field>": <value>, ... }`. Silcrow finds `[data-pilcrow-list="field"]` then `[data-pilcrow-key="row_key"]` within it, and calls `patch(changes, row)`.

## WebSocket Message Types

- `patch` — payload: `{ type: "patch", target, data[, mutation_id] }`. Same confirmation semantics as SSE patch.
- `html`
- `invalidate`
- `navigate`
- `custom`

---

## Custom DOM Events

- `silcrow:patched`
- `silcrow:navigate`
- `silcrow:before-swap`
- `silcrow:load`
- `silcrow:error`
- `silcrow:live:connect`
- `silcrow:live:disconnect`
- `silcrow:sse`
- `silcrow:sse:<event>`
- `silcrow:ws:<event>`
- `silcrow:optimistic` — fired when `publishOptimistic` applies a speculative patch
- `silcrow:confirmed` — fired when `confirmOptimistic` retires a pending mutation
- `silcrow:revert` — fired when `revertOptimistic` restores the snapshot

---

## Public API (`window.Silcrow`)

**Runtime**
- `patch`
- `invalidate`
- `stream`

**Navigation**
- `go`

**Live**
- `live`
- `send`
- `disconnect`
- `reconnect`

**Headless Store**
- `prefetch`
- `submit`
- `subscribe`
- `snapshot`
- `publish`

**Feedback**
- `publishOptimistic(scope, data, mutationId)` — snapshot atom + apply optimistic patch + register pending mutation
- `confirmOptimistic(mutationId)` — retire a pending mutation (server confirmed)
- `revertOptimistic(mutationId)` — restore atom snapshot and retire pending mutation
- `onToast`

**Extensibility**
- `use`
- `onRoute`
- `onError`

**Lifecycle**
- `destroy`

---

## Atom Scopes

- `route:<pathname>`
- `stream:<url>`
- `<custom-name>`

## SSR Hydration Globals

- `window.__silcrow_seed`
- `window.__pilcrow_props`

---

## Internal Subsystems

**Debug** — `DEBUG`, `warn`, `throwErr`

**URL Safety** — `URL_SAFE_PROTOCOLS`, `URL_ATTRS`, `SAFE_DATA_IMAGE_RE`, `hasSafeProtocol`, `hasSafeSrcSet`

**Safety** — `extractHTML`, `FORBIDDEN_HTML_TAGS`, `hardenBlankTargets`, `sanitizeTree`, `safeSetHTML`

**Toasts** — `processToasts`, `setToastHandler`, `silcrow_toasts` cookie

**Atoms** — `BLOCKED_ATOM_KEYS`, `isPlainMergeable`, `mergePath`, `createAtom`, `routeAtoms`, `streamAtoms`, `scopeAtoms`, `getOrCreateAtom`, `resolveAtomByScope`, `prefetchPromises`, `prefetchRoute`, `evictPrefetch`, `submitAction`, `bindElementToScope`, `unbindElementAtoms`, `initScopeBindings`, `seedAtomsFromSSR`

**Patcher** — `instanceCache`, `validatedTemplates`, `localBindingsCache`, `identityMap`, `patchMiddleware`, `PATH_RE`, `isValidPath`, `knownProps`, `URL_BINDING_PROPS`, `BLOCKED_KEYS`, `resolvePath`, `resolveRoot`, `getStableId`, `safeClone`, `parseForExpression`, `setValue`, `parseBind`, `scanBindings`, `reconcile`, `patchItem`, `mergeOrRemoveItem`, `buildMaps`, `patch`, `invalidate`, `stream`

**Live (SSE)** — `liveConnections`, `liveConnectionsByUrl`, `sseHubs`, `MAX_BACKOFF`, `isLikelyLiveUrl`, `normalizeSSEEndpoint`, `resolveLiveTarget`, `applyLivePatchPayload`, `pauseLiveState`, `resolveLiveStates`, `onSSEEvent`, `createSseHub`, `getOrCreateSseHub`, `removeSseHub`, `openLive`, `unsubscribeSse`, `connectSseHub`, `disconnectLive`, `reconnectLive`, `destroyAllLive`, `initLiveElements`

**WebSocket** — `wsHubs`, `normalizeWsEndpoint`, `createWsHub`, `getOrCreateWsHub`, `removeWsHub`, `connectWsHub`, `dispatchWsMessage`, `unsubscribeWs`, `openWsLive`, `sendWs`

**Navigator** — `VERB_ATTRS`, `VERB_SELECTOR`, `FORM_VERB_SELECTOR`, `DEFAULT_TIMEOUT`, `CACHE_TTL`, `MAX_CACHE`, `abortMap`, `routeHandler`, `errorHandler`, `responseCache`, `preloadInflight`, `resolveVerb`, `getTarget`, `getTimeout`, `showLoading`, `hideLoading`, `cacheSet`, `cacheGet`, `bustCacheOnMutation`, `processSideEffectHeaders`, `collectLayoutPatterns`, `buildFetchOptions`, `processResponseHeaders`, `prepareSwapContent`, `finalizeNavigation`, `navigate`, `onClick`, `onSubmit`, `onPopState`, `onMouseEnter`

**PS Fragment** — `extractHeadTemplate`, `applyHeadTemplate`, `parseFragmentSlot`, `applyFragment`, `resolveBoostTarget`

**Optimistic** — `pendingMutations` (Map mutationId→{scope,snapshot}), `pendingByScope` (Map scope→Set<mutationId>), `publishOptimistic`, `confirmOptimistic`, `revertOptimistic`, `scopeForTarget`, `hasPendingMutationForTarget`

**Lifecycle** — `liveObserver`, `middlewareLocked`, `init`, `destroy`, auto-boot on `DOMContentLoaded`

**CSS Classes** — `silcrow-loading`

**ARIA** — `aria-busy`
