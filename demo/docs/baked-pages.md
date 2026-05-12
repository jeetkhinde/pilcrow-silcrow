# Baked Pages Sandbox Model

This sandbox keeps baked pages as the primary serving model. SSR/load remains the
source-of-truth renderer: build-time baking calls it before traffic arrives, and
lazy baking calls it on first traffic.

## Core Model

- `BakedRouteDeclaration` declares two independent choices:
  - bake timing: `BuildTime`, `LazyOnFirstHit`, or `NeverBake`
  - artifact strategy: `FullPage` or `FragmentComposed { layout_key }`
- `FullPage` stores one complete HTML response. It is the simple strategy for
  small routes and the current ticket POC default.
- `FragmentComposed` stores the page body separately and composes it with a
  shared baked layout at serve time. This lets layout/header/footer changes flow
  into many pages without rebaking every page body.
- Slot patching is shared by both strategies.

## Artifact Layout

The sandbox writes artifacts under `.pilcrow-baked/`:

- `pages/<route>.html` stores full-page artifacts for simple routes.
- `pages/<route>/body.html` stores fragment-composed page bodies.
- `pages/<route>/metadata.json` stores route-shaped page metadata.
- `layouts/<key>.html` stores shared baked layouts.
- `fragments/<key>.html` stores shared baked fragments.
- `reverse-index.json` maps dependency keys to page/slot targets.

Route-shaped body and metadata paths intentionally mirror source route shape.
That makes generated artifacts easier to inspect, diff, and debug than a flat
hash-only cache.

## Request Lifecycle

- First lazy request: `get_or_render_declared()` misses, runs SSR/load, writes
  the declared artifact strategy, then serves the baked artifact.
- Prebaked request: `prebake_declared()` already wrote the artifact and metadata;
  the first GET is a hit and skips SSR/load.
- Stale request: stale metadata suppresses the hit, SSR/load reruns, and the
  refreshed artifact replaces the stale one.
- Composed-page request: the store reads page metadata, reads `body.html`, reads
  the declared layout, inserts the body into the layout `page_body` HTML slot,
  and returns the composed HTML.

## Mutation Lifecycle

1. A mutable value is named by a `DependencyKey`.
2. Baking records which page slots depend on that key in `reverse-index.json`.
3. A mutation uses `BakedPatchRegistry` to find affected page/slot targets.
4. The registered recompute function returns a new `SlotValue`.
5. The store patches the owning artifact: full-page HTML for `FullPage`, body
   HTML for `FragmentComposed`.
6. If recompute or marker validation fails, the page is marked stale so the next
   request falls back to SSR/load.

## Safety Rules

- Text slots always escape their replacement content.
- Trusted HTML requires an explicit `TrustedHtml` wrapper.
- Slot patching validates marker presence, uniqueness, order, and kind.
- Replacements cannot contain Pilcrow slot markers.
- Artifact and metadata writes use atomic temp-file replacement.

