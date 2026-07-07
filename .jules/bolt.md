## 2025-02-14 - [Silcrow JS Runtime Optimizations]
**Learning:** The silcrow headless store implementation (`silcrow/src/silcrow.js` in `mergePath`) was shallow copying previous state tree objects *before* actually detecting any differences in nested paths. When identical API payloads (very common in subscriptions/polling) are processed, this causes unnecessary allocations and GC overhead proportional to the tree size.

**Action:** When implementing structural sharing/immutable state updates in vanilla JS, always defer shallow object cloning until a mutation is definitively detected. If iterating properties finds no change, return the original reference (`prev`) directly without having allocated a throwaway clone.

## 2025-02-17 - [Silcrow JS DOM Sanitization Optimization]
**Learning:** The silcrow client-side runtime (`silcrow/src/silcrow.js` in `sanitizeTree`) was performing multiple DOM traversals using `querySelectorAll` for each forbidden tag and then again for all elements. This caused unnecessary DOM scanning overhead proportional to the tree size and the number of forbidden tags.
**Action:** When sanitizing DOM trees, consolidate traversals into a single pass over all elements (`querySelectorAll("*")`) and perform checks (forbidden tags, namespace validation) sequentially within that single loop to reduce performance cost.

## 2025-05-23 - [Silcrow JS Initialization Optimizations]
**Learning:** During framework initialization (`initLiveElements`) and high-frequency events (`mouseenter` for `startPreload`), multiple sequential `document.querySelectorAll()` calls or function invocations that trigger DOM scans create unnecessary performance overhead proportional to the DOM size.

**Action:** Consolidate multiple sequential `document.querySelectorAll` calls targeting the same subtree into a single comma-separated selector query. For high-frequency events, ensure functions returning DOM queries (`collectLayoutPatterns`) are cached in a local variable instead of being re-invoked within the same execution context.

## 2025-05-24 - [Silcrow JS DOM Sanitization Optimization (Optimistic Updates)]
**Learning:** During optimistic state publishing (`publishOptimistic`) and reverting (`revertOptimistic`), the silcrow runtime was performing multiple sequential `document.querySelectorAll` passes per mutated key to update live fields. For objects with multiple keys, this caused N full DOM scans.
**Action:** When updating multiple DOM elements based on a dictionary/object of keys, consolidate traversals into a single pass over all target elements (e.g. `querySelectorAll("[data-pilcrow-live-field]")`) and match their attributes against the data keys, rather than executing queries per key inside a loop.
