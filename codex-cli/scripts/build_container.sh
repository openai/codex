#!/bin/bash
# ------------------------------------------------------------
# Codex build and packaging script.
# Automates dependency installation, build, packaging,
# and Docker image creation.
# ------------------------------------------------------------

# Exit immediately on:
# -e : error (non-zero status)
# -u : use of undefined variable
# -o pipefail : failure anywhere in a pipe
set -euo pipefail

# ------------------------------------------------------------
# Determine the absolute path of the directory where this script resides.
# SCRIPT_DIR stores the resolved location.
#
# Logical:
#   - Ensures commands run relative to the script location, not user’s PWD.
#
# Electronic:
#   - The shell queries the filesystem via syscalls (open, stat)
#     to resolve absolute paths.
# ------------------------------------------------------------
SCRIPT_DIR=$(realpath "$(dirname "$0")")

# ------------------------------------------------------------
# trap ensures cleanup: when the script exits (any reason),
# "popd" is executed, restoring the previous directory.
#
# Logical:
#   - Acts as a "finally" block in scripting.
#
# Electronic:
#   - The kernel executes `popd` in this shell process
#     to update the directory stack in memory.
# ------------------------------------------------------------
trap "popd >> /dev/null" EXIT

# ------------------------------------------------------------
# Change to the parent directory of the script’s location.
# pushd also saves current dir in the directory stack.
#
# Logical:
#   - All subsequent commands (pnpm, docker) run from repo root.
#   - If cd fails, print error and exit.
#
# Electronic:
#   - Uses chdir() syscall to update process working directory.
# ------------------------------------------------------------
pushd "$SCRIPT_DIR/.." >> /dev/null || {
  echo "Error: Failed to change directory to $SCRIPT_DIR/.."
  exit 1
}

# ------------------------------------------------------------
# Install dependencies via pnpm (fast Node.js package manager).
#
# Logical:
#   - Downloads packages listed in package.json and lockfile.
#
# Electronic:
#   - pnpm resolves versions, fetches tarballs from registry,
#     writes them to local filesystem, updates symlinks.
# ------------------------------------------------------------
pnpm install

# ------------------------------------------------------------
# Run the build step defined in package.json scripts.
#
# Logical:
#   - Usually compiles TypeScript, bundles JS, or generates assets.
#
# Electronic:
#   - Node.js executes a new process (child) that runs build commands.
# ------------------------------------------------------------
pnpm run build

# ------------------------------------------------------------
# Clean up old distribution archives.
#
# Logical:
#   - Ensures no outdated package tarballs exist in dist/.
#
# Electronic:
#   - rm makes unlink() syscalls to remove files from filesystem.
# ------------------------------------------------------------
rm -rf ./dist/openai-codex-*.tgz

# ------------------------------------------------------------
# Create a new package tarball with pnpm pack.
# Output goes into ./dist directory.
#
# Logical:
#   - Produces a distributable .tgz archive of the package.
#
# Electronic:
#   - pnpm reads project files, writes them to tar archive,
#     compresses with gzip, and saves to disk.
# ------------------------------------------------------------
pnpm pack --pack-destination ./dist

# ------------------------------------------------------------
# Rename generated tarball to a consistent filename (codex.tgz).
#
# Logical:
#   - Provides a stable name regardless of version number.
#
# Electronic:
#   - mv issues rename() syscall on the filesystem.
# ------------------------------------------------------------
mv ./dist/openai-codex-*.tgz ./dist/codex.tgz

# ------------------------------------------------------------
# Build Docker image tagged "codex" using Dockerfile.
#
# Logical:
#   - Docker reads ./Dockerfile and creates a container image
#     containing the Codex build and runtime environment.
#
# Electronic:
#   - Docker daemon communicates with kernel namespaces,
#     builds filesystem layers, and stores image in local registry.
# ------------------------------------------------------------
docker build -t codex -f "./Dockerfile" .
