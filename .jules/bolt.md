## 2025-02-14 - [Silcrow JS Runtime Optimizations]
**Learning:** The silcrow headless store implementation (`silcrow/src/silcrow.js` in `mergePath`) was shallow copying previous state tree objects *before* actually detecting any differences in nested paths. When identical API payloads (very common in subscriptions/polling) are processed, this causes unnecessary allocations and GC overhead proportional to the tree size.

**Action:** When implementing structural sharing/immutable state updates in vanilla JS, always defer shallow object cloning until a mutation is definitively detected. If iterating properties finds no change, return the original reference (`prev`) directly without having allocated a throwaway clone.

## 2025-02-17 - [Silcrow JS DOM Sanitization Optimization]
**Learning:** The silcrow client-side runtime (`silcrow/src/silcrow.js` in `sanitizeTree`) was performing multiple DOM traversals using `querySelectorAll` for each forbidden tag and then again for all elements. This caused unnecessary DOM scanning overhead proportional to the tree size and the number of forbidden tags.
**Action:** When sanitizing DOM trees, consolidate traversals into a single pass over all elements (`querySelectorAll("*")`) and perform checks (forbidden tags, namespace validation) sequentially within that single loop to reduce performance cost.

## 2025-05-23 - [Silcrow JS Initialization Optimizations]
**Learning:** During framework initialization (`initLiveElements`) and high-frequency events (`mouseenter` for `startPreload`), multiple sequential `document.querySelectorAll()` calls or function invocations that trigger DOM scans create unnecessary performance overhead proportional to the DOM size.

**Action:** Consolidate multiple sequential `document.querySelectorAll` calls targeting the same subtree into a single comma-separated selector query. For high-frequency events, ensure functions returning DOM queries (`collectLayoutPatterns`) are cached in a local variable instead of being re-invoked within the same execution context.
## 2023-10-27 - Silcrow JS DOM query optimization
**Learning:** In the Silcrow client runtime (silcrow.js), using `querySelectorAll` inside tight loops like optimistic mutation patches or SSE message parsing creates severe performance bottlenecks because it performs O(N) full document scans per loop iteration (e.g., once per changed key).
**Action:** Always extract `querySelectorAll` calls out of loops and replace them with a single `querySelectorAll("[data-pilcrow-live-field], [data-s-live-mod]")` DOM scan. Iterate over the retrieved nodes and conditionally check the specific properties or attributes needed.
