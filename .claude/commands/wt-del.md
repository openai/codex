---
allowed-tools: Bash(git:*), Bash(rm:*)
description: Delete a worktree and its associated local branch
argument-hint: <branch-name>
---

## Context

- Current branch: !`git branch --show-current`
- Project directory: !`pwd | xargs basename`
- Existing worktrees: !`git worktree list`
- Branch to delete: $ARGUMENTS

## Task

Delete the worktree for branch `$ARGUMENTS` and remove the associated local branch.

### Step 1: Validate Input

1. Check that branch name is provided in `$ARGUMENTS`
2. If empty, report error: "Usage: /wt-del <branch-name>"
3. Verify we are NOT currently in the worktree being deleted

### Step 2: Locate Worktree

1. Get project name: `PROJECT=$(basename "$(pwd)")`
2. Convert branch name to safe directory name: replace `/` with `-`
3. Calculate expected worktree path: `../worktrees/${PROJECT}-${SAFE_BRANCH}`
4. Verify worktree exists at that path or find it via `git worktree list`

### Step 3: Remove Worktree

1. Remove the worktree: `git worktree remove <path>`
2. If worktree has uncommitted changes:
   - Report the situation
   - Ask user whether to force remove: `git worktree remove --force <path>`
3. Prune stale worktree info: `git worktree prune`

### Step 4: Delete Local Branch

1. Try safe delete: `git branch -d $ARGUMENTS`
2. If branch is not fully merged:
   - Report: "Branch has unmerged commits"
   - Show unmerged commits: `git log main..$ARGUMENTS --oneline`
   - Force delete: `git branch -D $ARGUMENTS`

### Step 5: Report Result

Report:
- Worktree path removed
- Branch deleted
- Remaining worktrees: `git worktree list`
