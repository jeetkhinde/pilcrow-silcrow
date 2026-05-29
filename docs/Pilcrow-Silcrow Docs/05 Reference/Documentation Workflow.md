# Documentation Workflow

## Source Order

Use this order when writing or updating docs:

1. `pilcrow/registry.toml`
2. MCP `list_features` and `explain_feature`
3. Dedicated local docs
4. Source refs from the registry
5. Tests and examples

For Silcrow public API docs, use `silcrow/docs/silcrow-api.md` first.

## Page Template

Each feature reference page should answer:

- What it is.
- When to use it.
- Minimal example.
- File conventions.
- Configuration.
- Constraints.
- Common mistakes.
- Related features.
- Source or test evidence when behavior is subtle.

Each teaching page should be task-first:

- What you will build.
- File layout.
- Step-by-step edits.
- Complete code blocks that can be copied into the project.
- Rules immediately after the code they explain.
- Expected validation errors and common mistakes.
- Link to the deeper reference page.

Avoid pages that only summarize a feature list. A reader should be able to build something after reading a teaching page.

## Maintenance Checklist

When a feature changes, update:

1. Implementation.
2. `pilcrow/registry.toml`.
3. MCP knowledge coverage when relevant.
4. Runnable test or executable example.
5. This docs vault.

After code or docs changes, run:

```bash
graphify update .
```
