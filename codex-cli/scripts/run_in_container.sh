#!/bin/bash

# Usage:
#   ./run_in_container.sh [--work_dir directory] "COMMAND"
#
#   Examples:
#     ./run_in_container.sh --work_dir project/code "ls -la"
#     ./run_in_container.sh "echo Hello, world!"

# Default the work directory to WORKSPACE_ROOT_DIR if not provided.
WORK_DIR="${WORKSPACE_ROOT_DIR}"

# Parse optional flag.
if [ "$1" = "--work_dir" ]; then
  if [ -z "$2" ]; then
    echo "Error: --work_dir flag provided but no directory specified."
    exit 1
  fi
  WORK_DIR="$2"
  shift 2
fi

# Ensure a command is provided.
if [ "$#" -eq 0 ]; then
  echo "Usage: $0 [--work_dir directory] \"COMMAND\""
  exit 1
fi

# Check if WORK_DIR is set.
if [ -z "$WORK_DIR" ]; then
  echo "Error: No work directory provided and WORKSPACE_ROOT_DIR is not set."
  exit 1
fi

# Remove any existing container named 'vibe'.
docker rm -f vibe || true

# Run the container with the specified directory mounted at the same path inside the container.
docker run --name vibe -d \
  -e OPENAI_API_KEY \
  --cap-add=NET_ADMIN \
  --cap-add=NET_RAW \
  -v "$WORK_DIR:$WORK_DIR" \
  vibe \
  sleep infinity

# Initialize the firewall inside the container.
docker exec vibe bash -c "sudo /usr/local/bin/init_firewall.sh"

# Execute the provided command in the container, ensuring it runs in the work directory.
# We use a parameterized bash command to safely handle the command and directory.
docker exec vibe bash -c "cd \"$WORK_DIR\" && vibe --dangerously-auto-approve-everything -q \"$@\""
