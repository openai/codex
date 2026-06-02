#!/bin/sh
set -eu

# Bazel supports per-user settings through the ignored user.bazelrc file imported
# by the repository's .bazelrc. Codex worktrees are separate checkouts, so ignored
# files from the main checkout are not present when a worktree is created. Copy the
# main checkout's user.bazelrc into each new worktree when one exists, while keeping
# the setup optional for contributors who do not use local Bazel overrides.
#
# See codex-rs/docs/bazel.md for the repository's Bazel workflow.

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
worktree_root="$(git -C "$script_dir/../.." rev-parse --path-format=absolute --show-toplevel)"
common_git_dir="$(git -C "$worktree_root" rev-parse --path-format=absolute --git-common-dir)"
main_checkout="$(dirname "$common_git_dir")"
source_path="$main_checkout/user.bazelrc"
destination_path="$worktree_root/user.bazelrc"

printf 'Codex environment setup:\n'
printf '  worktree: %s\n' "$worktree_root"
printf '  main checkout: %s\n' "$main_checkout"
printf '  source: %s\n' "$source_path"
printf '  destination: %s\n' "$destination_path"

if [ "$source_path" = "$destination_path" ]; then
  printf '  result: running in the main checkout; nothing to copy\n'
elif [ ! -f "$source_path" ]; then
  printf '  result: source does not exist; nothing to copy\n'
else
  cp -p "$source_path" "$destination_path"
  printf '  result: copied user.bazelrc\n'
fi
