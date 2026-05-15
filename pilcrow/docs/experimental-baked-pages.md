# Experimental Baked Pages

Baked Pages are Pilcrow's experimental durable serving artifact model. SSR/load
is still the source-of-truth renderer; baking only decides when that renderer is
called and where the resulting HTML is stored.

Enable the API with:

```toml
pilcrow-web = { path = "...", features = ["experimental-baked-pages"] }
```

## Timing Policies

- `BuildTime`: framework or app code calls `BakedPageStore::prebake_declared`
  before traffic. A request only serves an existing fresh artifact.
- `LazyOnFirstHit`: the first request checks the store, renders on miss, writes
  the artifact, and later requests hit the baked artifact.
- `NeverBake`: the route renders normally and never writes or serves a baked
  artifact.

## Artifact Modes

- `FullPage`: stores one complete HTML file for the concrete path.
- `FragmentComposed { layout_key }`: stores the route-shaped page body
  separately and composes it into the named layout's `page_body` slot at serve
  time. This lets shared layout changes affect responses without rebaking page
  bodies.

## Slots And Dependencies

A `DependencyKey` declares which domain object owns a slot. Patch operations use
the reverse index to find pages and slots affected by a key, recompute only the
declared slot, and patch the artifact atomically.

Text slots escape HTML by default:

```rust
SlotValue::text("<b>Closed</b>")
```

Trusted HTML must be explicit:

```rust
SlotValue::trusted_html(TrustedHtml::new("<strong>Closed</strong>"))
```

Marker validation is strict: one start marker, one end marker, and the start
must appear before the end. Recompute or patch failures mark the affected page
stale.

## Lazy Serving

`BakedRoute` is the smallest web-layer opt-in. It wraps a store and a
`BakedRouteDeclaration`, then delegates to `BakedPageStore::get_or_render_declared`.

```rust,ignore
use std::io;

use pilcrow_web::experimental::baked_pages::{
    serve_baked_or_render, BakedPageStore, BakedRenderedPage, BakedRoute,
    BakedRouteDeclaration, DependencyKey,
};
use pilcrow_web::axum::response::Response;

fn ticket_route() -> BakedRoute {
    let store = BakedPageStore::new(".pilcrow-baked");
    let declaration = BakedRouteDeclaration::lazy_on_first_hit(
        "/tickets/:id",
        "/tickets/123",
    )
    .full_page()
    .text_slot("ticket_status", vec![DependencyKey::new("ticket:123")]);

    BakedRoute::new(store, declaration)
}

async fn ticket_handler(route: BakedRoute) -> io::Result<Response> {
    route.serve(|_declaration| {
        Ok(BakedRenderedPage::new(
            r#"<html>
              <body>
                <!--pilcrow-slot:start ticket_status kind=text-->Open<!--pilcrow-slot:end ticket_status-->
              </body>
            </html>"#,
            "render-v1",
        ))
    })
}

async fn ticket_handler_without_wrapper() -> io::Result<Response> {
    let store = BakedPageStore::new(".pilcrow-baked");
    let declaration = BakedRouteDeclaration::lazy_on_first_hit(
        "/tickets/:id",
        "/tickets/123",
    )
    .fragment_composed("app")
    .text_slot("ticket_status", vec![DependencyKey::new("ticket:123")]);

    serve_baked_or_render(&store, &declaration, |_declaration| {
        Ok(BakedRenderedPage::new(
            "<!--pilcrow-slot:start ticket_status kind=text-->Open<!--pilcrow-slot:end ticket_status-->",
            "render-v1",
        ))
    })
}
```

For `FragmentComposed`, the layout artifact must exist and contain:

```html
<!--pilcrow-slot:start page_body kind=html--><!--pilcrow-slot:end page_body-->
```

## Prebake

`BakedPageStore::prebake_declared` accepts a `BuildTime` declaration and a render
function. It writes the same metadata, artifact, and dependency mapping that lazy
baking writes on first request. `LazyOnFirstHit` is not prebaked, and `NeverBake`
is rejected.

There is a tiny experimental example that prebakes one `BuildTime + FullPage`
route and one `BuildTime + FragmentComposed` route, then serves both as
`hit/skipped` without running the request renderer:

```bash
cargo run -p pilcrow-web --features experimental-baked-pages --example baked_prebake
```

There is also an experimental ticket example that mirrors the sandbox story: a
lazy full-page ticket route, a lazy fragment-composed summary route, an explicit
`DependencyKey`, and a mutation endpoint that patches the baked slot so the next
GET serves updated HTML without rendering again:

```bash
cargo run -p pilcrow-web --features experimental-baked-pages --example baked_ticket
```

## Patching

`BakedPatchRegistry` registers recompute functions by slot name. When called with
a `DependencyKey`, it reads the reverse index, recomputes each affected slot,
patches the page artifact, and marks the page stale if recompute or marker
validation fails.

## Current Limitations

- Experimental feature flag only.
- Explicit opt-in only.
- No global router or routekit codegen integration yet.
- No production CLI yet.
- No TTL, ISR integration, database triggers, production storage adapter, or
  distributed invalidation.
