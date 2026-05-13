
# Agent Instructions

Reading order:
1. Claude.md (workspace root) — project topology, build commands, crate map
2. AGENTS.md (this file) — agent-specific instructions
3. pilcrow/CLAUDE.md — Pilcrow framework guidance
4. silcrow/CLAUDE.md — Silcrow JS runtime guidance
5. pilcrow/AGENTS.md — Pilcrow-specific agent rules, if present
6. silcrow/AGENTS.md — Silcrow-specific agent rules, if present

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- ALWAYS read graphify-out/GRAPH_REPORT.md before reading any source files, running grep/glob searches, or answering codebase questions. The graph is your primary map of the codebase.
- IF graphify-out/wiki/index.md EXISTS, navigate it instead of reading raw files
- For cross-module "how does X relate to Y" questions, prefer `graphify query "<question>"`, `graphify path "<A>" "<B>"`, or `graphify explain "<concept>"` over grep — these traverse the graph's EXTRACTED + INFERRED edges instead of scanning files
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
