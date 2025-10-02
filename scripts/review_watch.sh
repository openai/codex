#!/usr/bin/env bash
set -euo pipefail

REPO="${UPSTREAM_REPO:-openai/codex}"
PR="${PR_NUMBER:-4522}"
MARKER="[review-watch]"

echo "Fetching PR $REPO#$PR status..."
json=$(gh pr view "$PR" -R "$REPO" --json number,title,state,isDraft,mergeable,reviewDecision,updatedAt,headRefName,baseRefName,url,statusCheckRollup)

# Build a concise summary
state=$(jq -r '.state' <<<"$json")
mergeable=$(jq -r '.mergeable' <<<"$json")
review=$(jq -r '.reviewDecision' <<<"$json")
updated=$(jq -r '.updatedAt' <<<"$json")
title=$(jq -r '.title' <<<"$json")
url=$(jq -r '.url' <<<"$json")

checks=$(jq -r '.statusCheckRollup[] | "- " + (.workflowName // .name) + ": " + ((.conclusion // .status) // "PENDING")' <<<"$json" 2>/dev/null || true)
[ -z "$checks" ] && checks="(no checks reported)"

body=$(cat <<EOF
$MARKER PR status summary
- Title: $title
- State: $state
- Mergeable: $mergeable
- Review: $review
- Updated: $updated
- Link: $url

Checks:
$checks

Notes:
- If checks are "Expected — Waiting" from a fork, a maintainer must approve workflows to run.
- I will update this comment when the status meaningfully changes.
EOF
)

# Find last bot comment with marker
comments=$(gh api -X GET "/repos/$REPO/issues/$PR/comments?per_page=100")
last=$(jq -r --arg m "$MARKER" '[.[] | select(.body|contains($m))][-1].body' <<<"$comments" 2>/dev/null || echo "null")

if [ "$last" != "null" ] && [ "$last" = "$body" ]; then
  echo "No change since last summary; not commenting."
  exit 0
fi

echo "Posting status summary comment..."
gh pr comment "$PR" -R "$REPO" -b "$body"
