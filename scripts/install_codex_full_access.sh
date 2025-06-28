#!/bin/bash

###############################################################################
# File Name: install_codex_full_access.sh
# Purpose:   Complete setup of OpenAI Codex CLI on a local machine with no
#            network or execution restrictions. It installs dependencies,
#            builds Codex, sets up a convenient alias, verifies installation,
#            and cleans up all installation files.
# Author:    Your Name
# Date:      2025-06-27
# Context:   For beginner programmers who want Codex to run freely and safely
#            outside Docker, with robust error handling and easy debugging.
###############################################################################

# Enable strict error handling
set -euo pipefail

# Log file for debugging
LOG_FILE="$HOME/codex_install_debug.log"
exec > >(tee -a "$LOG_FILE") 2>&1

echo "===== Codex Full-Access Installation Script ====="
echo "All output is logged to $LOG_FILE for debugging."
echo "If you encounter errors, check this log for details."
echo

# Step 1: Reset Firewall (Optional, only if previously restricted)
if command -v iptables &>/dev/null; then
  echo "[1/7] Resetting firewall to allow all access (requires sudo)..."
  sudo iptables -F || { echo "Warning: Failed to flush iptables filter rules."; }
  sudo iptables -X || { echo "Warning: Failed to delete iptables chains."; }
  sudo iptables -t nat -F || { echo "Warning: Failed to flush nat rules."; }
  sudo iptables -t nat -X || { echo "Warning: Failed to delete nat chains."; }
  sudo iptables -t mangle -F || { echo "Warning: Failed to flush mangle rules."; }
  sudo iptables -t mangle -X || { echo "Warning: Failed to delete mangle chains."; }
  sudo iptables -P INPUT ACCEPT || { echo "Warning: Failed to set INPUT policy."; }
  sudo iptables -P FORWARD ACCEPT || { echo "Warning: Failed to set FORWARD policy."; }
  sudo iptables -P OUTPUT ACCEPT || { echo "Warning: Failed to set OUTPUT policy."; }
  echo "Firewall reset complete."
else
  echo "iptables not found, skipping firewall reset."
fi
echo

# Step 2: Check Prerequisites
echo "[2/7] Checking for prerequisites: git, node, npm, pnpm, rust..."

required_cmds=(git node npm curl)
for cmd in "${required_cmds[@]}"; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "Error: $cmd is required but not installed. Please install it and rerun."
    exit 1
  fi
done

if ! command -v pnpm &>/dev/null; then
  echo "pnpm not found. Installing pnpm globally via npm..."
  npm install -g pnpm || { echo "Failed to install pnpm. Aborting."; exit 1; }
  echo "Adding npm global binary path to PATH"
  export PATH="$(npm bin -g):$PATH"
fi

if ! command -v cargo &>/dev/null; then
  echo "Rust (cargo) not found. Installing Rust (this may take a few minutes)..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  export PATH="$HOME/.cargo/bin:$PATH"
fi

echo "All prerequisites are installed."
echo

# Step 3: Clone Codex Repository
echo "[3/7] Cloning the OpenAI Codex repository..."
if [ -d "$HOME/codex" ]; then
  echo "Codex directory already exists at $HOME/codex. Updating repository..."
  cd "$HOME/codex"
  git pull || { echo "Failed to update Codex repo. Aborting."; exit 1; }
else
  git clone https://github.com/openai/codex.git "$HOME/codex" || { echo "Failed to clone Codex repo. Aborting."; exit 1; }
  cd "$HOME/codex"
fi
echo "Repository ready."
echo

# Step 4: Build & Install Codex CLI
echo "[4/7] Installing dependencies and building Codex CLI..."
cd "$HOME/codex/codex-cli"
pnpm install || { echo "pnpm install failed. Check $LOG_FILE for details."; exit 1; }
pnpm build   || { echo "pnpm build failed. Check $LOG_FILE for details."; exit 1; }

echo "Running native dependency install..."
bash scripts/install_native_deps.sh || { echo "Native dependency install failed. Aborting."; exit 1; }
echo "Build and installation complete."
# Create a 'codex' shim pointing to the real entrypoint, for CLI invocations without .js
cd "$HOME/codex/codex-cli/bin"
if [ -f codex.js ] && [ ! -e codex ]; then
  ln -s codex.js codex || echo "Warning: failed to create 'codex' symlink"
fi
echo

# Step 5: Add Alias to Shell Configuration
echo "[5/7] Creating 'cx' alias for Codex CLI in your ~/.zshrc..."

ZSHRC="$HOME/.zshrc"
CLI_PATH="$HOME/codex/codex-cli/bin/codex"

if ! grep -q "alias cx=" "$ZSHRC" 2>/dev/null; then
  echo "alias cx=\"$CLI_PATH\"" >> "$ZSHRC"
  echo "Alias 'cx' added to $ZSHRC."
else
  echo "Alias 'cx' already present in $ZSHRC. Skipping."
fi
echo "To use 'cx' in this terminal, run: source $ZSHRC"
echo

# Step 6: Verify Installation
echo "[6/7] Verifying Codex installation..."

# skip alias here; test the CLI binary directly

if "$CLI_PATH" --help &>/dev/null; then
  echo "Codex CLI is working!"
else
  echo "Error: Codex CLI did not respond as expected. Check $LOG_FILE for details."
  exit 1
fi

echo "Verification complete."
echo

# Step 7: Cleanup Installation Files
echo "[7/7] Cleaning up installation files..."

find "$HOME/codex" -type f -name "pnpm-debug.log" -delete
find "$HOME/codex" -type f -name "npm-debug.log" -delete
rm -rf "$HOME/codex/codex-cli/dist" || true

echo "Cleanup complete."
echo

echo "===== Codex installation and setup is COMPLETE! ====="
echo "You can now use 'cx' in a new terminal session. Try 'cx --help' to get started."
echo "If you encounter issues, review the log at $LOG_FILE."
