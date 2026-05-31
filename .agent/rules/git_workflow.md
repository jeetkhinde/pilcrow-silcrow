## Git Workflow & Branch Protection

To keep the `main` branch clean and stable, you must adhere to the following git workflow rules:

- **No Direct Commits/Pushes to `main`**:
  - You are strictly forbidden from committing or pushing code directly to the `main` branch.
  - All changes must be developed in a feature branch and merged via a Pull Request (PR) process.

- **Mandatory Git Worktrees**:
  - When starting a new feature or task, you must create and work within a git `worktree` (e.g., `git worktree add -b <branch-name> <path-to-worktree>`).
  - This keeps the main workspace directory clean and prevents unwanted local modifications to `main`.

- **PR Review and Validation Process**:
  - When a PR is opened and proposed for merging into `main`, you must check:
    - AI agent advice, feedback, and automated pipeline results on the PR.
    - Your own self-reviews and code validation of the PR.
  - The `main` branch must be kept clean, building, and green at all times.
