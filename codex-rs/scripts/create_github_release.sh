#!/bin/bash

set -euo pipefail

# By default, this script uses a version based on the current date and time.
# If you want to specify a version, pass it as the first argument. Example:
#
#     ./scripts/create_github_release.sh 0.1.0-alpha.4
#
# The value will be used to update the `version` field in `Cargo.toml`.

# Change to the root of the Cargo workspace.
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# Cancel if there are uncommitted changes.
if ! git diff --quiet || ! git diff --cached --quiet || [ -n "$(git ls-files --others --exclude-standard)" ]; then
  echo "ERROR: You have uncommitted or untracked changes." >&2
  exit 1
fi

# Fail if in a detached HEAD state.
CURRENT_BRANCH=$(git symbolic-ref --short -q HEAD 2>/dev/null || true)
if [ -z "${CURRENT_BRANCH:-}" ]; then
  echo "ERROR: Could not determine the current branch (detached HEAD?)." >&2
  echo "       Please run this script from a checked-out branch." >&2
  exit 1
fi

# Ensure we are on the 'main' branch before proceeding.
if [ "${CURRENT_BRANCH}" != "main" ]; then
  echo "ERROR: Releases must be created from the 'main' branch (current: '${CURRENT_BRANCH}')." >&2
  echo "       Please switch to 'main' and try again." >&2
  exit 1
fi

# Ensure the current local commit on 'main' is present on 'origin/main'.
# This guarantees we only create releases from commits that are already on
# the canonical repository (https://github.com/openai/codex).
if ! git fetch --quiet origin main; then
  echo "ERROR: Failed to fetch 'origin/main'. Ensure the 'origin' remote is configured and reachable." >&2
  exit 1
fi

if ! git merge-base --is-ancestor HEAD origin/main; then
  echo "ERROR: Your local 'main' HEAD commit is not present on 'origin/main'." >&2
  echo "       Please push your commits first (git push origin main) or check out a commit on 'origin/main'." >&2
  exit 1
fi

# Create a new branch for the release and make a commit with the new version.
if [ $# -ge 1 ]; then
  VERSION="$1"
else
  VERSION=$(printf '0.0.%d' "$(date +%y%m%d%H%M)")
fi
TAG="rust-v$VERSION"
git checkout -b "$TAG"
perl -i -pe "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
git add Cargo.toml
git commit -m "Release $VERSION"
git tag -a "$TAG" -m "Release $VERSION"
git push origin "refs/tags/$TAG"

git checkout "$CURRENT_BRANCH"
