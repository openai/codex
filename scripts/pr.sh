#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/pr.sh <command> [args]
Commands:
  prepare <branch> <body_md>   Format, fix, tests; push branch; create/update PR body
  comment <pr> <md>            Post PR comment from a markdown file
  body <pr> <md>               Update PR body from a markdown file
  trigger                      Create empty commit and push to retrigger Codex Review
USAGE
}

cmd=${1:-}
case "$cmd" in
  prepare)
    branch=${2:?branch}; body=${3:?body_md}
    echo "Formatting & targeted fixes..."; (cd codex-rs && just fmt || true)
    # Optional: add targeted crate fixes here if desired.
    echo "Pushing branch $branch..."
    if git show-ref --verify --quiet "refs/heads/$branch"; then
      git checkout -q "$branch"
    else
      git checkout -q -b "$branch"
    fi
    git add -A && git commit -m "chore: prepare PR $branch" || true
    git push --set-upstream origin "$branch" || true
    echo "Creating/updating PR body..."
    gh pr view --json number >/dev/null 2>&1 || gh pr create -F "$body" || true
    gh pr edit -F "$body" || true
    ;;
  comment)
    pr=${2:?pr}; md=${3:?md}
    gh pr comment "$pr" -F "$md"
    ;;
  body)
    pr=${2:?pr}; md=${3:?md}
    gh pr edit "$pr" -F "$md"
    ;;
  trigger)
    git commit --allow-empty -m "chore: re-run Codex Review" && git push --force-with-lease
    ;;
  *)
    usage; exit 1;;
esac

