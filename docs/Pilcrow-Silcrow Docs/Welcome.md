# Pilcrow-Silcrow Docs

This vault documents Pilcrow, the Rust SSR framework, and Silcrow, the client runtime bundled with it.

Start here:

- [[00 Start Here/Documentation Map]]
- [[00 Start Here/Learning Path]]
- [[00 Start Here/Feature Status]]
- [[03 Rendering/Build an FSR Page]]
- [[01 Pilcrow/Mental Model]]
- [[01 Pilcrow/Routing]]
- [[01 Pilcrow/Pages and Layouts]]
- [[03 Rendering/Live Props and FSR]]
- [[02 Silcrow/Silcrow Runtime]]

Authoritative local sources:

- `pilcrow/registry.toml` is the feature contract index.
- `pilcrow/tools/pilcrow-mcp` is the AI-facing feature evidence layer.
- `.claude/rendering-models.md` is the current rendering-mode reference.
- `silcrow/docs/silcrow-api.md` is the Silcrow public API reference.

When these docs conflict with the registry or MCP evidence, update the docs and then fix the stale source in the same follow-up change.
