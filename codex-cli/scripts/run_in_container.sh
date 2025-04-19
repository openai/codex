#!/bin/bash
set -e

# Usage:
#   ./run_in_container.sh [--dangerously-allow-network-outbound] [--work_dir directory] "COMMAND"
#
#   Examples:
#     ./run_in_container.sh --work_dir project/code "ls -la"
#     ./run_in_container.sh "echo Hello, world!"

# Default the work directory to WORKSPACE_ROOT_DIR if not provided.
WORK_DIR="${WORKSPACE_ROOT_DIR:-$(pwd)}"
# By default, do not disable outbound firewall
ALLOW_OUTBOUND=false

# Parse optional flags: --dangerously-allow-network-outbound and --work_dir
while [ "$#" -gt 0 ]; do
  case "$1" in
    --dangerously-allow-network-outbound)
      ALLOW_OUTBOUND=true
      shift
      ;;
    --work_dir)
      if [ -z "${2:-}" ]; then
        echo "Error: --work_dir flag provided but no directory specified."
        exit 1
      fi
      WORK_DIR="$2"
      shift 2
      ;;
    *)
      break
      ;;
  esac
done

WORK_DIR=$(realpath "$WORK_DIR")

# Generate a unique container name based on the normalized work directory
CONTAINER_NAME="codex_$(echo "$WORK_DIR" | sed 's/\//_/g' | sed 's/[^a-zA-Z0-9_-]//g')"

# Define cleanup to remove the container on script exit, ensuring no leftover containers
cleanup() {
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
}
# Trap EXIT to invoke cleanup regardless of how the script terminates
trap cleanup EXIT

# Ensure a command is provided.
if [ "$#" -eq 0 ]; then
  echo "Usage: $0 [--dangerously-allow-network-outbound] [--work_dir directory] \"COMMAND\""
  exit 1
fi

# Check if WORK_DIR is set.
if [ -z "$WORK_DIR" ]; then
  echo "Error: No work directory provided and WORKSPACE_ROOT_DIR is not set."
  exit 1
fi

# Kill any existing container for the working directory using cleanup(), centralizing removal logic.
cleanup

# Run the container with the specified directory mounted at the same path inside the container.
docker run --name "$CONTAINER_NAME" -d \
  -e OPENAI_API_KEY \
  --cap-add=NET_ADMIN \
  --cap-add=NET_RAW \
  -v "$WORK_DIR:/app$WORK_DIR" \
  codex \
  sleep infinity

# Initialize the firewall inside the container, passing allow-outbound if requested
if [ "$ALLOW_OUTBOUND" = true ]; then
  echo "Initializing firewall inside container (allowing outbound traffic)..."
  docker exec "$CONTAINER_NAME" bash -c "sudo /usr/local/bin/init_firewall.sh --dangerously-allow-network-outbound"
else
  echo "Initializing firewall inside container..."
  docker exec "$CONTAINER_NAME" bash -c "sudo /usr/local/bin/init_firewall.sh"
fi

# Execute the provided command in the container, ensuring it runs in the work directory.
# We use a parameterized bash command to safely handle the command and directory.

quoted_args=""
for arg in "$@"; do
  quoted_args+=" $(printf '%q' "$arg")"
done
docker exec -it "$CONTAINER_NAME" bash -c "cd \"/app$WORK_DIR\" && codex --full-auto ${quoted_args}"
