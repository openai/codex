---
allowed-tools: Bash(git:*), Read, Edit, AskUserQuestion
description: Merge a worktree branch into current branch, resolve conflicts if any
argument-hint: <branch-name>
---

## Context

- Current branch: !`git branch --show-current`
- Git status: !`git status --short`
- Branch to merge: $ARGUMENTS
- Available branches: !`git branch --list`

## Task

Merge branch `$ARGUMENTS` into the current branch. Handle conflicts intelligently.

### Step 1: Validate Input

1. Check that branch name is provided in `$ARGUMENTS`
2. If empty, report error: "Usage: /wt-merge <branch-name>"
3. Verify the branch exists: `git rev-parse --verify $ARGUMENTS`

### Step 2: Pre-merge Check

1. Check for uncommitted changes in current branch
2. If dirty, ask user whether to:
   - Stash changes before merge
   - Commit changes first
   - Abort merge

### Step 3: Execute Merge

1. Show commits to be merged: `git log HEAD..$ARGUMENTS --oneline`
2. Execute merge: `git merge $ARGUMENTS`
3. Check merge result

### Step 4: Handle Conflicts (if any)

If merge conflicts occur:

1. List conflicted files: `git diff --name-only --diff-filter=U`
2. For each conflicted file:
   - Read the file to understand the conflict
   - Analyze both sides of the conflict (ours vs theirs)
   - Attempt to resolve if the resolution is clear:
     - Simple additions from both sides → combine them
     - Same change on both sides → keep one
   - For complex conflicts that cannot be auto-resolved:
     - Show the conflict to user
     - Ask user for resolution preference
3. After resolving all conflicts:
   - Stage resolved files: `git add <file>`
   - Complete merge: `git commit -m "Merge branch '$ARGUMENTS'"`

### Step 5: Report Result

Report:
- Merge status (success/conflicts resolved)
- Number of commits merged
- Files changed: `git diff --stat HEAD~1`
- Any conflicts that were resolved and how
