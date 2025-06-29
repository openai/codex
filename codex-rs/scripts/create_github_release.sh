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
CURRENT_BRANCH=$(git symbolic-ref --short -q HEAD)

# Create a new branch for the release and make a commit with the new version.
if [ $# -ge 1 ]; then
  VERSION="$1"
else
  VERSION=$(printf '0.0.%d' "$(date +%y%m%d%H%M)")
fi
TAG="rust-v$VERSION"
RELEASE_BRANCH="release/$TAG"

git checkout -b "$TAG"
perl -i -pe "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
git add Cargo.toml
git commit -m "Release $VERSION"
git tag -a "$TAG" -m "Release $VERSION"

# The commit identified by the tag must be reachable from a branch so that
# when GitHub creates the `Source code (tar.gz)` for the release, it can find
# the commit. This is a requirement for Homebrew to be able to install the
# package from the tarball.
git push origin "$RELEASE_BRANCH"
git push origin "refs/tags/$TAG"

git checkout "$CURRENT_BRANCH"
