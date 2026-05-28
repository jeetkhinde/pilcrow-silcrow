## 2025-02-14 - [Silcrow JS Runtime Optimizations]
**Learning:** The silcrow headless store implementation (`silcrow/src/silcrow.js` in `mergePath`) was shallow copying previous state tree objects *before* actually detecting any differences in nested paths. When identical API payloads (very common in subscriptions/polling) are processed, this causes unnecessary allocations and GC overhead proportional to the tree size.

**Action:** When implementing structural sharing/immutable state updates in vanilla JS, always defer shallow object cloning until a mutation is definitively detected. If iterating properties finds no change, return the original reference (`prev`) directly without having allocated a throwaway clone.

## 2025-05-27 - [Optimize DOM Queries in JS Runtime]
**Learning:** Sequential `querySelectorAll` passes for specific attributes (e.g., live connections, bindings) result in redundant complete sub-tree scans. In a framework handling many live nodes, this can cause noticeable main thread blocking during initialization, hydration, and cleanup.
**Action:** Always combine multiple selectors into a single `querySelectorAll` query when traversing the DOM for framework wiring. Process attributes dynamically on the resulting matched elements to minimize layout/traversal overhead.
