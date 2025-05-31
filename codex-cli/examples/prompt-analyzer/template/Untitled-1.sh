#!/bin/bash
set -euo pipefail

echo "Setting up development environment for codex-cli..."

# Install Node.js 22.x
if ! command -v node &> /dev/null || [[ $(node --version | cut -d. -f1 | tr -d 'v') -lt 22 ]]; then
  echo "Installing Node.js 22.x..."
  curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
  sudo apt-get install -y nodejs
fi

# Verify Node.js version
node_version=$(node --version)
echo "Node.js version: $node_version"

# Install pnpm
if ! command -v pnpm &> /dev/null; then
  echo "Installing pnpm..."
  curl -fsSL https://get.pnpm.io/install.sh | sh -
  export PNPM_HOME="${HOME}/.local/share/pnpm"
  export PATH="${PNPM_HOME}:${PATH}"

  # Add pnpm to PATH in /etc/profile
  echo 'export PNPM_HOME="${HOME}/.local/share/pnpm"' | sudo tee -a /etc/profile
  echo 'export PATH="${PNPM_HOME}:${PATH}"' | sudo tee -a /etc/profile
fi

# Navigate to the project directory
cd /mnt/persist/workspace/codex-cli

# Install dependencies using pnpm
echo "Installing project dependencies..."
pnpm install

# Build the project
echo "Building the project..."
pnpm run build

echo "Setup completed successfully!"
