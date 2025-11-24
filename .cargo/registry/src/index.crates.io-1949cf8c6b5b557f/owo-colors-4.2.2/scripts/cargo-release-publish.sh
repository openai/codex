#!/bin/bash

# Use cargo-release to publish crates to crates.io.

set -xe -o pipefail

# cargo-release requires a release off a branch (maybe it shouldn't?)
# Check out this branch, creating it if it doesn't exist.
git checkout -B to-release

# --execute: actually does the release
# --no-confirm: don't ask for confirmation, since this is a non-interactive script
cargo release publish --publish --execute --no-confirm --workspace "$@"

git checkout -
git branch -D to-release
