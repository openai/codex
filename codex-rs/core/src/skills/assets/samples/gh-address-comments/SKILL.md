---
name: gh-address-comments
description: Help address review/issue comments on the open GitHub PR for the current branch using gh CLI; verify gh auth first and prompt the user to run oai_gh if not logged in.
---

# PR Comment Handler

Guide to find the open PR for the current branch and address its comments with gh CLI.

## 1) Inspect comments needing attention
- Run codex/skills/address-comments/scripts/fetch_comments.py which will print out all the comments and review threads on the PR

## 2) Ask the user for clarification
- Number all the review threads and comments and provide a short summary of what would be required to apply a fix for it
- Ask the user which numbered comments should be addressed

## 3) If user chooses comments
- Apply fixes for the selected comments

Notes:
- If gh hits auth/rate issues mid-run, prompt the user to re-run `oai_gh`, then retry.
