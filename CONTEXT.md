# Pilcrow + Silcrow Integration Context

## Architecture
- Pilcrow = Rust SSR engine
- Silcrow = client JS runtime

## Integration
- silcrow/src/*.js is source of truth.
- Pilcrow embeds it via:
  crates/runtime/assets/silcrow.js

## Build Rule
- NEVER manually edit pilcrow's silcrow.js
- Always generated from silcrow build

## Notes
- Keep API between SSR and client aligned