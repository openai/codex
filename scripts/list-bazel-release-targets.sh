#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

printf '%s\n' \
  "//codex-rs/..." \
  "-//codex-rs/v8-poc:all"
