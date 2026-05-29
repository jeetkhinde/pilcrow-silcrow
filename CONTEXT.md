# Pilcrow + Silcrow Integration Context

## Architecture
- Pilcrow = Rust SSR engine
- Silcrow = client JS runtime
- address-book = consumer Pilcrow app in this workspace (FSR reference app)

## Integration
- silcrow/src/*.js is source of truth.
- Pilcrow embeds it via:
  crates/runtime/assets/silcrow.js
- address-book/Cargo.toml depends on Pilcrow by path and exercises routekit as a
  consumer app outside the Pilcrow repo.

## Build Rule
- NEVER manually edit pilcrow's silcrow.js
- Always generated from silcrow npm run build
- Build the consumer app from this workspace root with:
  `cargo build --manifest-path address-book/Cargo.toml`

## Notes
- Keep API between SSR and client aligned
