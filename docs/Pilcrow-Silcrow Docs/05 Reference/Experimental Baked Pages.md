# Experimental Baked Pages

Baked Pages are an experimental durable serving artifact model.

The current API exposes declaration types, filesystem-backed artifact storage, metadata storage, slot patching, prebake helpers, and explicit lazy serving helpers.

It does not automatically change generated routes.

## Enable

```toml
pilcrow-web = { path = "...", features = ["experimental-baked-pages"] }
```

## Concepts

Timing policies:

- `BuildTime`
- `LazyOnFirstHit`
- `NeverBake`

Artifact strategies:

- `FullPage`
- `FragmentComposed`

## Constraints

- Experimental library API only.
- Routes must explicitly call the baked serving helper.
- No generated route or router behavior changes automatically.
- Build-time prebake is library-only.
- Storage is local filesystem only.
- Trusted HTML slot patches require explicit trusted HTML wrappers.

Use this feature only when experimenting with durable HTML artifacts and slot-level patching.
