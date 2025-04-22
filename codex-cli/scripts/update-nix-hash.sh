#!/usr/bin/env bash

export REPO_ROOT="$(git rev-parse --show-toplevel)"

# Run nix build command and capture output
build_output=$(nix build .\#codex-cli --show-trace 2>&1)

# Extract the "got" hash using grep and awk
# Look for the line containing "got:" and extract the hash
NEW_HASH=$(echo "$build_output" | grep -A 1 "hash mismatch" | grep "got:" | awk '{print $2}')

# Check if we found a hash
if [ -n "$NEW_HASH" ]; then
    echo "Extracted got hash: $NEW_HASH"
else
    echo "Could not extract got hash from build output."
    echo "Full build output:"
    echo "$build_output"
fi
# Update the hash in flake.nix
sed -i "s|hash = \"sha256-[^\"]*\"|hash = \"$NEW_HASH\"|" "$REPO_ROOT/flake.nix"

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
  git commit -m "chore: Update pnpm deps hash in flake.nix

This automated commit updates the pnpm deps hash hash in flake.nix to match 
the latest hash of the workspace's pnpm-lock.yaml file. This ensures that 
Nix's store is consistent with the latest dependencies."

  git push
else
  echo "Hash has been updated in flake.nix."
  echo "You may want to commit this change with:"
  echo "  git add $REPO_ROOT/flake.nix"
  echo "  git commit -m \"chore: Update npmDepsHash in flake.nix\""
fi

echo "Successfully updated npmDepsHash in flake.nix"
