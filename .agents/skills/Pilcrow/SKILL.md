---
name: "Pilcrow"
description: "Pilcrow-Silcrow development, testing, and git worktree PR workflow instructions"
---

# Pilcrow Framework Git Worktree & PR Workflow

This skill contains instructions and best practices for developing, testing, and merging features in the Pilcrow repository using git worktrees and GitHub PRs.

## Workflow Rules

### 1. Mandatory Git Worktrees
Before touching any code files, always create a worktree branch:
```bash
git worktree add -b <branch-name> .worktrees/<branch-name>
```
Develop, test, and commit your changes entirely within this worktree.

### 2. Git Push and PR Creation
Once your work is committed inside the worktree, push the feature branch to GitHub and create a pull request:
```bash
git push origin <branch-name>
gh pr create --title "<PR Title>" --body "<PR Description>" --base main --head <branch-name>
```

### 3. Merging Pull Requests (Checkout Resolution)
A common error when using `gh pr merge` from within a worktree is:
```
failed to run git: fatal: 'main' is already used by worktree at ...
```
This happens because `gh` attempts to check out `main` locally, which is already checked out by the parent repository.

**Resolution:**
Use the GitHub API directly to perform the merge without local checkouts:
```bash
gh api -X PUT repos/<owner>/<repo>/pulls/<pr_number>/merge -f merge_method=merge
```

### 4. Cleanup
After the PR is merged:
1. Delete the remote branch:
   ```bash
   git push origin --delete <branch-name>
   ```
2. Move back to the parent repository directory.
3. Remove the local worktree:
   ```bash
   git worktree remove .worktrees/<branch-name> --force
   ```
4. Delete the local branch:
   ```bash
   git branch -D <branch-name>
   ```
5. Pull `main` in the parent repository:
   ```bash
   git pull origin main
   ```
