#!/usr/bin/env bash

set -euo pipefail

manifest="${1:?missing zsh manifest path}"
expected_sha256="${2:?missing expected zsh manifest SHA-256}"

if command -v sha256sum >/dev/null 2>&1; then
  printf '%s  %s\n' "$expected_sha256" "$manifest" | sha256sum --check -
else
  printf '%s  %s\n' "$expected_sha256" "$manifest" | shasum -a 256 --check -
fi
