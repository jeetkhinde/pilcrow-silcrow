# Pilcrow + Silcrow Integration Context

## Architecture
- Pilcrow = Rust SSR engine
- Silcrow = client JS runtime
- sandbox = external production-style Pilcrow app in this workspace

## Integration
- silcrow/src/*.js is source of truth.
- Pilcrow embeds it via:
  crates/runtime/assets/silcrow.js
- sandbox/Cargo.toml depends on Pilcrow by path and exercises routekit as a
  consumer app outside the Pilcrow repo.

## Build Rule
- NEVER manually edit pilcrow's silcrow.js
- Always generated from silcrow build
- Build sandbox from this workspace root with:
  `cargo build --manifest-path sandbox/Cargo.toml`

## Notes
- Keep API between SSR and client aligned
