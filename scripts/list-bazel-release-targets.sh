#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

# Keep this list focused on first-party Rust targets whose compile surface can
# differ when `cfg(not(debug_assertions))` becomes active.
#
# Exclude the experimental `v8-poc` target because it pulls in expensive V8
# build machinery that is unrelated to the release-only Rust regression this
# workflow is meant to catch.
printf '%s\n' \
  "//codex-rs/..." \
  "-//codex-rs/v8-poc:all"
