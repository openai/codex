#!/bin/bash

set -euo pipefail

# Change to the root of the Cargo workspace.
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# Cancel if there are uncommitted changes.
if ! git diff --quiet || ! git diff --cached --quiet || [ -n "$(git ls-files --others --exclude-standard)" ]; then
  echo "ERROR: You have uncommitted or untracked changes." >&2
  exit 1
fi

# Fail if in a detached HEAD state.
CURRENT_BRANCH=$(git symbolic-ref --short -q HEAD)

# Create a new branch for the release and make a commit with the new version.
VERSION=$(printf '0.0.%d' "$(date +%y%m%d%H%M)")
TAG="rust-v$VERSION"
git checkout -b "$TAG"
perl -i -pe "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
git add Cargo.toml
git commit -m "Release $VERSION"
git tag -a "$TAG" -m "Release $VERSION"
git push origin "refs/tags/$TAG"
git checkout "$CURRENT_BRANCH"
