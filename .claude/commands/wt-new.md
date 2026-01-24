---
allowed-tools: Bash(git:*), Bash(mkdir:*)
description: Create a new git worktree in ../worktrees/ directory
argument-hint: <branch-name>
---

## Context

- Current branch: !`git branch --show-current`
- Project directory: !`pwd | xargs basename`
- Existing worktrees: !`git worktree list`
- Branch name: $ARGUMENTS

## Task

Create a new git worktree for branch `$ARGUMENTS` in the parent worktrees directory.

### Step 1: Validate Input

1. Check that branch name is provided in `$ARGUMENTS`
2. If empty, report error: "Usage: /wt-new <branch-name>"

### Step 2: Calculate Paths

1. Get project name: `PROJECT=$(basename "$(pwd)")`
2. Convert branch name to safe directory name: replace `/` with `-`
   - Example: `fix/test` → `fix-test`
3. Calculate worktree path: `../worktrees/${PROJECT}-${SAFE_BRANCH}`

### Step 3: Create Worktree

1. Create worktrees directory if not exists: `mkdir -p ../worktrees`
2. Check if branch already exists:
   - If exists: `git worktree add <path> <branch>` (checkout existing branch)
   - If not exists: `git worktree add <path> -b <branch>` (create new branch)
3. Handle errors (e.g., worktree already exists)

### Step 4: Report Result

Report:
- Created worktree path
- Branch name
- Based on commit (show `git log -1 --oneline` for the new worktree)
- How to switch: `cd <path>`
