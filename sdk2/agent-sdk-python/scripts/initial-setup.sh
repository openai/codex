#!/bin/bash

# Initial setup script for installing git hooks

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Setting up git hooks..."

# Install pre-push hook
echo "→ Installing pre-push hook..."
cp "$SCRIPT_DIR/pre-push" "$REPO_ROOT/.git/hooks/pre-push"
chmod +x "$REPO_ROOT/.git/hooks/pre-push"
echo "✓ pre-push hook installed"

echo ""
echo "✓ Setup complete!"
echo ""
echo "The pre-push hook will now run lint checks before each push."
echo "To skip the hook temporarily, use: git push --no-verify"
