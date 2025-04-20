#!/bin/bash
set -e

# Usage:
#   ./run_in_container.sh [--work_dir directory] [--allowed_domains "domain1 domain2 ..."] "COMMAND"
#
#   Examples:
#     ./run_in_container.sh --work_dir project/code "ls -la"
#     ./run_in_container.sh --allowed_domains "api.openai.com api.anthropic.com" "echo Hello, world!"
#     ./run_in_container.sh "echo Hello, world!"

# Default the work directory to WORKSPACE_ROOT_DIR if not provided.
WORK_DIR="${WORKSPACE_ROOT_DIR:-$(pwd)}"
ALLOWED_DOMAINS="${ALLOWED_DOMAINS:-api.openai.com}"

# Parse optional flags.
while [[ $# -gt 0 ]]; do
  case "$1" in
    --work_dir)
      if [ -z "$2" ]; then
        echo "Error: --work_dir flag provided but no directory specified."
        exit 1
      fi
      WORK_DIR="$2"
      shift 2
      ;;
    --allowed_domains)
      if [ -z "$2" ]; then
        echo "Error: --allowed_domains flag provided but no domains specified."
        exit 1
      fi
      ALLOWED_DOMAINS="$2"
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
  echo "Usage: $0 [--work_dir directory] [--allowed_domains \"domain1 domain2 ...\"] \"COMMAND\""
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
  -e ALLOWED_DOMAINS="$ALLOWED_DOMAINS" \
  --cap-add=NET_ADMIN \
  --cap-add=NET_RAW \
  -v "$WORK_DIR:/app$WORK_DIR" \
  codex \
  sleep infinity

# Convert space-separated domains to array of arguments for init_firewall.sh
domain_args=()
for domain in $ALLOWED_DOMAINS; do
  domain_args+=("$domain")
done

# Initialize the firewall inside the container.
docker exec "$CONTAINER_NAME" bash -c "sudo /usr/local/bin/init_firewall.sh ${domain_args[*]}"

# Execute the provided command in the container, ensuring it runs in the work directory.
# We use a parameterized bash command to safely handle the command and directory.

quoted_args=""
for arg in "$@"; do
  quoted_args+=" $(printf '%q' "$arg")"
done
docker exec -it "$CONTAINER_NAME" bash -c "cd \"/app$WORK_DIR\" && codex --full-auto ${quoted_args}"
