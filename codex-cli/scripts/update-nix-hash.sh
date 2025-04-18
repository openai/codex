#!/usr/bin/env bash
set -euo pipefail

# Script to update the npmDepsHash in flake.nix when package-lock.json changes
# This script is meant to be triggered by a GitHub Action or run manually

# Find the repository root (where the .git directory is)
REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

# Check if package-lock.json has changed
if [ -z "$(git diff --name-only HEAD^ HEAD 2>/dev/null | grep 'codex-cli/package-lock.json')" ]; then
  echo "No changes to package-lock.json detected in the last commit."
  echo "Running manual hash update..."
fi

# Calculate the new hash
NEW_HASH=$(nix hash path "$REPO_ROOT/codex-cli" --type sha256)

# Update the hash in flake.nix
sed -i "s|npmDepsHash = \"sha256-[^\"]*\"|npmDepsHash = \"$NEW_HASH\"|" "$REPO_ROOT/flake.nix"

# Check if the hash was actually changed
if [ -z "$(git diff "$REPO_ROOT/flake.nix")" ]; then
  echo "Hash is already up to date. No changes needed."
  exit 0
fi

# If running in GitHub Actions environment, configure git user
if [ -n "${GITHUB_ACTIONS:-}" ]; then
  git config user.name "GitHub Actions"
  git config user.email "actions@github.com"

  # Commit and push the change
  git add "$REPO_ROOT/flake.nix"
  git commit -m "chore: Update npmDepsHash in flake.nix

This automated commit updates the npmDepsHash in flake.nix to match 
the latest package-lock.json changes."

  git push
else
  echo "Hash has been updated in flake.nix."
  echo "You may want to commit this change with:"
  echo "  git add $REPO_ROOT/flake.nix"
  echo "  git commit -m \"chore: Update npmDepsHash in flake.nix\""
fi

echo "Successfully updated npmDepsHash in flake.nix"
