---
description: Refresh workspace context — re-read CLAUDE.md, AGENTS.md, plans, and run MCP scan
allowed-tools: Read, mcp__pilcrow__scan_project_context
---

This is the ONLY sanctioned place to proactively call `scan_project_context`.
Run this when CLAUDE.md may be stale or when starting a session after a significant gap.

**Step 1** — Read the workspace context files:
- `/Users/jagjeet/Development/workspaces/pilcrow-silcrow/CLAUDE.md`
- `/Users/jagjeet/Development/workspaces/pilcrow-silcrow/AGENTS.md`
- `/Users/jagjeet/Development/workspaces/pilcrow-silcrow/CONTEXT.md`
- All files under `/Users/jagjeet/Development/workspaces/pilcrow-silcrow/plans/`

**Step 2** — Call `mcp__pilcrow__scan_project_context` to get the current MCP state.

**Step 3** — Report any discrepancies:
- Does CLAUDE.md's build command section match what the MCP scan reports?
- Are any features in the MCP registry that aren't mentioned in CLAUDE.md's known-issues section?
- Are the active plans still current based on the MCP feature statuses?

After running this command, CLAUDE.md is considered current for the session.
Do NOT call `scan_project_context` again unless you encounter a specific build failure.
