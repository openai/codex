---
name: code-review
description: Run a final code review on a pull request
---

Use subagents to review code using all code-review-* skills in this repository. One subagent per skill. Pass full skill path to subagents. Use xhigh reasoning.

You can return an unlimited number of findings. 
Use raw Markdown to report findings.
Number findings for the ease of reference.

If the GitHub user running the review is the owner of the pull request add a `code-reviewed` label.
Do not leave GitHub comments unless explicitly asked.

