#!/usr/bin/env python3
"""
Stop hook: checks whether MCP sync files and Silcrow docs were updated
alongside framework changes. Emits a systemMessage reminder if not.
Always exits 0 — this is a reminder, not a hard block.
"""
import json
import subprocess
import sys

PILCROW_ROOT = "/Users/jagjeet/Development/workspaces/pilcrow-silcrow/pilcrow"
SILCROW_ROOT = "/Users/jagjeet/Development/workspaces/pilcrow-silcrow/silcrow"

MCP_SYNC_FILES = {
    "registry.toml",
    "tools/mcp/pilcrow-mcp/src/validation.rs",
    "tools/mcp/pilcrow-mcp/src/docs.rs",
}

SILCROW_DOC_FILE = "docs/silcrow-api.md"


def git_changed_files(repo_root):
    result = subprocess.run(
        ["git", "-C", repo_root, "diff", "--name-only", "HEAD"],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        return set()
    return set(result.stdout.strip().splitlines())


def main():
    try:
        _input = json.load(sys.stdin)
    except Exception:
        sys.exit(0)

    messages = []

    # --- Pilcrow check ---
    pilcrow_changed = git_changed_files(PILCROW_ROOT)

    framework_files = {
        f for f in pilcrow_changed
        if f.startswith("crates/") or f.startswith("pages/") or f.startswith("src/")
    }
    # Exclude the MCP sync files themselves from "framework changed" count
    framework_files -= MCP_SYNC_FILES

    if framework_files:
        missing_sync = MCP_SYNC_FILES - pilcrow_changed
        if missing_sync:
            missing_list = ", ".join(sorted(missing_sync))
            messages.append(
                f"MCP sync incomplete. Framework files changed but not updated: {missing_list}. "
                f"Run: cargo test --manifest-path pilcrow/tools/mcp/pilcrow-mcp/Cargo.toml"
            )

    # --- Silcrow check ---
    silcrow_changed = git_changed_files(SILCROW_ROOT)

    silcrow_src_changed = {f for f in silcrow_changed if f.startswith("src/")}
    if silcrow_src_changed and SILCROW_DOC_FILE not in silcrow_changed:
        messages.append(
            f"Silcrow API docs not updated. silcrow/src/ changed but {SILCROW_DOC_FILE} was not. "
            f"Update docs/silcrow-api.md in the same commit."
        )

    if messages:
        output = {"systemMessage": " | ".join(messages)}
        print(json.dumps(output))

    sys.exit(0)


if __name__ == "__main__":
    main()
