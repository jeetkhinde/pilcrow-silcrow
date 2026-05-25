## 2025-02-14 - [Silcrow JS Runtime Optimizations]
**Learning:** The silcrow headless store implementation (`silcrow/src/silcrow.js` in `mergePath`) was shallow copying previous state tree objects *before* actually detecting any differences in nested paths. When identical API payloads (very common in subscriptions/polling) are processed, this causes unnecessary allocations and GC overhead proportional to the tree size.

**Action:** When implementing structural sharing/immutable state updates in vanilla JS, always defer shallow object cloning until a mutation is definitively detected. If iterating properties finds no change, return the original reference (`prev`) directly without having allocated a throwaway clone.

## 2025-05-25 - [Silcrow JS Runtime Optimizations]
**Learning:** Sequential `querySelectorAll` traversals over the same DOM subtree (especially `document.querySelectorAll`) cause significant redundant scanning overhead during initialization and mutation handling. In this codebase, multiple subsystems (live connections, SSE, Websockets, atoms bindings) often needed to scan the same elements.
**Action:** When optimizing DOM manipulation inside vanilla JS runtimes, consolidate separate attribute checks into a single combined `querySelectorAll("...selectors")` pass, then evaluate `element.hasAttribute` within the iteration. This minimizes document-wide scanning overhead while preserving exact behavior.
