---
name: code-review
description: Run a final code review on a pull request
---

Use subagents to review code using all code-review-* skills other than this orchestrator.

One subagent per skill. Pass full skill path and review scope to subagents without forking any
shared turns or context.

You must return every single issue from every subagent. You can return an unlimited number of findings.
Use raw Markdown to report findings.
Number findings for ease of reference.
Each finding must include a specific file path and line number.

If the GitHub user running the review is the owner of the pull request add a `code-reviewed` label.
Do not leave GitHub comments unless explicitly asked.
