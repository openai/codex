---
allowed-tools: Bash(git:*)
description: Squash commits since a base commit with auto-generated message
argument-hint: <base-commit-id>
---

## Context

- Current branch: !`git branch --show-current`
- Git status: !`git status --short`
- Base commit: $ARGUMENTS

## Task

Execute git squash workflow for all commits from `$ARGUMENTS` to HEAD.

### Step 1: Pre-squash Safety

1. **Check for uncommitted changes**: If there are any uncommitted changes (staged or unstaged), commit them first with message `"wip: uncommitted changes before squash"`
2. **Backup to remote**: Push current branch to origin as backup: `git push origin <current-branch> -f`

### Step 2: Analyze Commits to be Squashed

Before squashing, gather information about all commits:

1. List commits: `git log --oneline $ARGUMENTS..HEAD`
2. Get detailed info with body: `git log --format="%h %s%n%b---" $ARGUMENTS..HEAD`
3. Get file change stats: `git log --oneline --stat $ARGUMENTS..HEAD`
4. Count total commits and record the stats for later verification

### Step 3: Execute Squash

1. Run `git reset --soft $ARGUMENTS` to squash all commits while keeping changes staged
2. Analyze:
   - All original commit messages and their intent
   - The staged changes (`git diff --cached --stat`)
   - Group related changes into logical categories
3. Generate an excellent commit message following conventional commits format:
   - **Title**: `<type>(<scope>): <concise summary>` (max 72 chars)
   - **Body**: Organized sections describing major changes by category
   - Types: feat, fix, refactor, test, docs, chore, perf
4. Create the squashed commit with the generated message

### Step 4: Review & Verify

1. Show new commit: `git log --oneline -3`
2. Verify changes preserved: `git diff $ARGUMENTS..HEAD --stat`
3. Compare with pre-squash stats:
   - Same number of files changed
   - Same total insertions/deletions
4. Report:
   - Original commit count
   - New single commit hash and message
   - Confirmation that all changes are preserved
   - Any discrepancies found (should be none)
