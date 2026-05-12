# Demo App Consolidation Design

**Date:** 2026-05-12  
**Branch:** feat/live-props  
**Status:** Approved

## Overview

Merge `sandbox/` and `pilcrow-demos/` into a single `demo/` app. The old rendering-mode demo pages (SSR, ISR, ISR+SSG, SSG, deferred, streaming) are deleted — not migrated — because FSR subsumes all of them through field-level composition. The combined app showcases FSR and React islands as the two primary Pilcrow features.

## Motivation

- FSR renders like SSG when no `LiveProp` is given, like ISR when a field has a revalidation interval, and like streaming/deferred for `AsyncValue` fields. Separate demo pages for each mode duplicate the concept without adding clarity.
- Two apps mean double the nav, double the config, double the DX surface to maintain. Fewer things = more focus.
- Framework cleanup (removing ISR/SSG/deferred internals) is a follow-on task, informed by what the demo app actually exercises.

## App Identity

| Property | Old (sandbox) | New (demo) |
|---|---|---|
| Directory | `sandbox/` | `demo/` |
| Cargo package | `sandbox-web` | `pilcrow-demo` |
| Port | 3000 (auto-fallback now built in) | 3000 (same) |
| Deleted | — | `pilcrow-demos/` entirely |

All existing sandbox config is preserved: React/Solid islands, i18n, image optimization, baked pages, widgets, fragments.

## Page Inventory

### Kept (from sandbox, unchanged)

| Route | Notes |
|---|---|
| `/` | Home/index — nav updated |
| `/about` | Static page |
| `/live` | LiveProp demo |
| `/tickets` | Live ticket board — primary FSR example |
| `/products` | Product grid with API route |
| `/(admin)/dashboard` | Admin group |
| `/(admin)/stats` | Admin group |

### Deleted (from pilcrow-demos, not migrated)

`/ssr`, `/isr`, `/isr-ssg`, `/ssg`, `/deferred`, `/streaming` — all superseded by FSR.

### Added

| Route | Purpose |
|---|---|
| `/demo/fsr` | FSR field-composition showcase |
| `/demo/react-island` | React island mount + server props showcase |

## `/demo/fsr` Page Design

Demonstrates that FSR composes rendering mode **per field**, not per page.

**Layout:** Three visible panels (side by side on desktop, stacked on mobile).

| Panel | Field type | Content |
|---|---|---|
| Static | Plain Rust value | Build-time value, never updates |
| Live | `LiveProp` | Polls and updates without full reload |
| Async | `AsyncValue` | Streamed in after initial HTML |

Each panel has:
- A **badge** labeling the field type
- A **timestamp or counter** making the update behavior immediately observable
- A one-line caption explaining what's happening

The Rust handler (`pages/demo/fsr/index.rs`) uses a plain value, a `LiveProp`, and an `AsyncValue` on the same struct to make field-level composition concrete.

## `/demo/react-island` Page Design

Demonstrates how a React island mounts into server-rendered HTML and receives props from Rust.

**Part 1 — Basic island mount**
- A simple interactive React component (counter or toggle) mounted via `data-pilcrow-react`
- Shows: server renders shell HTML, React hydrates the island client-side

**Part 2 — Island with server props**
- Same pattern, but the Rust handler passes typed data as island props
- Shows: props flow Rust → JSON → React at render time, no client-side fetch needed

Each part has:
- A label showing the `data-pilcrow-react` component name
- A short caption: "This counter is a React island — the surrounding page is plain HTML"
- Optional collapsible `<details>` showing the React source

Uses the existing `react/` directory (renamed from `sandbox/react/`) — no new React infrastructure needed.

## Nav & Layout

`_layout.html` nav gains two groups:

**App** — Home · About · Live · Tickets · Products · Admin

**Demos** — FSR · React Island

The two groups are visually separated (divider or label). All other layout files (`_loading.html`, `_error.html`, `_not_found.html`) are unchanged.

## Out of Scope

- Framework cleanup (removing ISR/SSG/deferred internals from pilcrow-runtime/routekit) — follow-on task after the demo app confirms what's safe to remove
- Solid island demo page — added as `/demo/solid-island` in a future iteration when Solid support is ready

## File Operations Summary

1. Rename `sandbox/` → `demo/`
2. Update `Cargo.toml` package name: `sandbox-web` → `pilcrow-demo`
3. Update `.code-workspace` path references
4. Add `demo/pages/demo/fsr/index.rs` + `index.html`
5. Add `demo/pages/demo/react-island/index.rs` + `index.html`
6. Add React component for island demo in `demo/react/`
7. Update `demo/pages/_layout.html` nav
8. Delete `pilcrow-demos/` directory
