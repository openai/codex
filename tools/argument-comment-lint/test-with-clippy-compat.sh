#!/usr/bin/env bash
set -euo pipefail

# Dylint 6.1.0 still checks out rust-clippy's removed master branch. Add an
# alias only to that clone until we upgrade to a Dylint version that uses main.
clippy_config="$(mktemp)"
trap 'rm -f -- "$clippy_config"' EXIT
git config --file "$clippy_config" remote.origin.fetch '+refs/heads/main:refs/heads/master'

config_index="${GIT_CONFIG_COUNT:-0}"
export "GIT_CONFIG_KEY_${config_index}=includeIf.hasconfig:remote.*.url:https://github.com/rust-lang/rust-clippy.path"
export "GIT_CONFIG_VALUE_${config_index}=$clippy_config"
GIT_CONFIG_COUNT="$((config_index + 1))" cargo test "$@"
