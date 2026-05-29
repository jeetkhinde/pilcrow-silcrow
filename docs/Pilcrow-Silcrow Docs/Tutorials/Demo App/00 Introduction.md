# Demo App Tutorial

A walkthrough of the Pilcrow demo app — a feature showcase that goes beyond the [[../Address Book/00 Introduction|Address Book tutorial]] to cover FSR multi-counter dashboards, scalar vs object live fields, API routes, React islands, and UI component imports.

**Prerequisite:** complete the [[../Address Book/00 Introduction|Address Book tutorial]] first. This tutorial assumes you understand file routing, layouts, named actions, and basic FSR slots.

The finished code lives in `demo/` in the workspace root.

## What the demo covers

| Section | New concepts |
|---|---|
| [[01 Setup and Layout]] | UI component imports, `import` frontmatter, `slot name=` |
| [[02 Basic SSR Pages]] | Static props, external API fetch with `reqwest` |
| [[03 FSR Dashboard]] | Multiple live counters from one SQL query, `pilcrow_fsr` multi-slot |
| [[04 Ticket Detail — Scalar vs Object]] | Object `LiveProp<T>`, `s-use="fsr.slot"`, the class trap |
| [[05 API Routes with FSR]] | `axum::Router`, `FsrStore` extension, `invalidate_dep_key` |
| [[06 React Islands]] | `<react src>`, strategies, server props from Rust |

## Steps

1. [[01 Setup and Layout]]
2. [[02 Basic SSR Pages]]
3. [[03 FSR Dashboard]]
4. [[04 Ticket Detail — Scalar vs Object]]
5. [[05 API Routes with FSR]]
6. [[06 React Islands]]
