---
name: yeet
description: Automate branch+PR flow: if not already, create ambrosino/{description} branch, then stage+commit all, push, and open a gh PR with a concise summary.
---

# Yeet
- If on main/master/default, `git checkout -b "ambrosino/{description}"`; otherwise stay on the current branch.
- Confirm status `git status -sb`, then stage everything `git add -A`.
- Commit tersely with the description, e.g. `git commit -m "{description}"`.
- Push with tracking: `git push -u origin $(git branch --show-current)`.
- Open a PR: `gh pr create --fill --head $(git branch --show-current)` and edit title/body to reflect the description and the deltas.
