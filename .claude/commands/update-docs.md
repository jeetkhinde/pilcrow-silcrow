---
description: Update MCP registry and docs after a Pilcrow or Silcrow feature change
allowed-tools: Read, Edit, Bash
---

Argument: `$ARGUMENTS` — feature name or short description of what changed.

If `$ARGUMENTS` is empty, ask which feature or area changed before proceeding.

---

## For Pilcrow framework changes

**Step 1** — Read `pilcrow/registry.toml`. Find the feature matching `$ARGUMENTS`.
Update stale fields: `status`, `spec`, `canonical_usage`, `constraints`, `source_refs`.

**Step 2** — Read `pilcrow/tools/pilcrow-mcp/src/validation.rs`.
Remove error rules that no longer apply. Add new validation rules for changed behavior.

**Step 3** — Read `pilcrow/tools/pilcrow-mcp/src/docs.rs`.
Add `DocumentSpec` entries for any new source files introduced by this change.

**Step 4** — Run the MCP test suite:
```bash
cargo test --manifest-path /Users/jagjeet/Development/workspaces/pilcrow-silcrow/pilcrow/tools/pilcrow-mcp/Cargo.toml
```
All golden tests must pass. `cargo check` is NOT sufficient.

---

## For Silcrow public API changes

**Step 1** — Confirm edits were made to `silcrow/src/` (not `dist/`).

**Step 2** — Run build:
```bash
cd /Users/jagjeet/Development/workspaces/pilcrow-silcrow/silcrow && npm run build
```

**Step 3** — Update `silcrow/docs/silcrow-api.md` to reflect the API change.

**Step 4** — Verify Pilcrow embeds the new build:
```bash
cargo build --manifest-path /Users/jagjeet/Development/workspaces/pilcrow-silcrow/pilcrow/Cargo.toml -p pilcrow-runtime
```

---

**Report**: Show the diff of all modified files and the test/build output.
State explicitly which steps passed and which (if any) still need attention.
