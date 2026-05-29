---
description: Verify the full silcrow → pilcrow → address-book build chain in sequence
allowed-tools: Bash
---

Run these steps in order. Stop and report on first failure — do not continue past a failing step.

**Step 1 — Build Silcrow JS runtime**
```bash
cd /Users/jagjeet/Development/workspaces/pilcrow-silcrow/silcrow && npm run build
```
Success: `dist/silcrow.js` is updated. Failure: show the npm error output.

**Step 2 — Build Pilcrow runtime crate** (triggers build.rs to copy silcrow.js)
```bash
cargo build --manifest-path /Users/jagjeet/Development/workspaces/pilcrow-silcrow/pilcrow/Cargo.toml -p pilcrow-runtime
```
Failure here usually means a silcrow API mismatch or missing node_modules. Show the first `error[E...]` from stderr.

**Step 3 — Build address-book consumer app**
```bash
cargo build --manifest-path /Users/jagjeet/Development/workspaces/pilcrow-silcrow/address-book/Cargo.toml
```
Failure here usually means address-book has a Pilcrow API incompatibility.

**Report format:**
- Step 1: ✓ / ✗ + first error line if failed
- Step 2: ✓ / ✗ + first error line if failed
- Step 3: ✓ / ✗ + first error line if failed
- If all pass: "Build chain OK — silcrow.js is embedded and address-book compiles."
