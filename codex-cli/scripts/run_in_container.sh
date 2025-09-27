#!/bin/bash
set -e

# ------------------------------------------------------------
# Usage examples:
#   ./run_in_container.sh [--work_dir directory] "COMMAND"
#
#   ./run_in_container.sh --work_dir project/code "ls -la"
#   ./run_in_container.sh "echo Hello, world!"
#
# Logical:
#   - Provides a wrapper for running Codex inside a Docker container.
#   - Ensures proper working directory, firewall rules, and domain restrictions.
#
# Electronic:
#   - Relies on Docker CLI syscalls (docker run/exec/rm).
#   - Uses trap to clean up container on process exit.
# ------------------------------------------------------------

# Default work directory → either WORKSPACE_ROOT_DIR env var or current directory.
WORK_DIR="${WORKSPACE_ROOT_DIR:-$(pwd)}"

# Default allowed domains → api.openai.com (can be overridden by env var).
OPENAI_ALLOWED_DOMAINS="${OPENAI_ALLOWED_DOMAINS:-api.openai.com}"

# ------------------------------------------------------------
# Parse optional --work_dir flag
# Logical:
#   - If --work_dir is provided, override default WORK_DIR.
# Electronic:
#   - Bash argument shifting ($1, $2, shift).
# ------------------------------------------------------------
if [ "$1" = "--work_dir" ]; then
  if [ -z "$2" ]; then
    echo "Error: --work_dir flag provided but no directory specified."
    exit 1
  fi
  WORK_DIR="$2"
  shift 2
fi

WORK_DIR=$(realpath "$WORK_DIR")

# ------------------------------------------------------------
# Generate unique container name
# Logical:
#   - Names container based on normalized work dir.
# Electronic:
#   - sed filters characters to alphanumeric and underscores.
# ------------------------------------------------------------
CONTAINER_NAME="codex_$(echo "$WORK_DIR" | sed 's/\//_/g' | sed 's/[^a-zA-Z0-9_-]//g')"

# ------------------------------------------------------------
# Cleanup function
# Logical:
#   - Removes container on script exit.
# Electronic:
#   - docker rm -f terminates and deletes container process.
# ------------------------------------------------------------
cleanup() {
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# ------------------------------------------------------------
# Input validation
# Logical:
#   - Must provide command and valid WORK_DIR.
#   - Must ensure allowed domains are set.
# ------------------------------------------------------------
if [ "$#" -eq 0 ]; then
  echo "Usage: $0 [--work_dir directory] \"COMMAND\""
  exit 1
fi

if [ -z "$WORK_DIR" ]; then
  echo "Error: No work directory provided and WORKSPACE_ROOT_DIR is not set."
  exit 1
fi

if [ -z "$OPENAI_ALLOWED_DOMAINS" ]; then
  echo "Error: OPENAI_ALLOWED_DOMAINS is empty."
  exit 1
fi

# Kill any existing container for this work directory.
cleanup

# ------------------------------------------------------------
# Start container
# Logical:
#   - Runs codex container detached with mounted work dir.
#   - Adds network capabilities for firewall setup.
# Electronic:
#   - docker run creates new Linux namespace (PID, NET, MNT).
#   - -v mounts host path into container filesystem.
# ------------------------------------------------------------
docker run --name "$CONTAINER_NAME" -d \
  -e OPENAI_API_KEY \
  --cap-add=NET_ADMIN \
  --cap-add=NET_RAW \
  -v "$WORK_DIR:/app$WORK_DIR" \
  codex \
  sleep infinity

# ------------------------------------------------------------
# Configure allowed domains inside container
# Logical:
#   - Writes domains to /etc/codex/allowed_domains.txt.
#   - Ensures validation to prevent shell injection.
# Electronic:
#   - docker exec runs commands in running container as root.
#   - chmod/chown set file permissions on ext4/xfs inside container.
# ------------------------------------------------------------
docker exec --user root "$CONTAINER_NAME" bash -c "mkdir -p /etc/codex"
for domain in $OPENAI_ALLOWED_DOMAINS; do
  if [[ ! "$domain" =~ ^[a-zA-Z0-9][a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$ ]]; then
    echo "Error: Invalid domain format: $domain"
    exit 1
  fi
  echo "$domain" | docker exec --user root -i "$CONTAINER_NAME" bash -c "cat >> /etc/codex/allowed_domains.txt"
done

docker exec --user root "$CONTAINER_NAME" bash -c "chmod 444 /etc/codex/allowed_domains.txt && chown root:root /etc/codex/allowed_domains.txt"

# ------------------------------------------------------------
# Firewall initialization
# Logical:
#   - Initializes iptables rules inside container.
#   - Deletes script after execution for security.
# Electronic:
#   - docker exec calls /usr/local/bin/init_firewall.sh in container namespace.
#   - rm removes script from container filesystem.
# ------------------------------------------------------------
docker exec --user root "$CONTAINER_NAME" bash -c "/usr/local/bin/init_firewall.sh"
docker exec --user root "$CONTAINER_NAME" bash -c "rm -f /usr/local/bin/init_firewall.sh"

# ------------------------------------------------------------
# Execute user command inside container
# Logical:
#   - Changes into mounted work dir.
#   - Runs codex CLI with user-provided arguments.
# Electronic:
#   - docker exec -it attaches interactive terminal (TTY).
#   - bash -c executes concatenated command string.
# ------------------------------------------------------------
quoted_args=""
for arg in "$@"; do
  quoted_args+=" $(printf '%q' "$arg")"
done
docker exec -it "$CONTAINER_NAME" bash -c "cd \"/app$WORK_DIR\" && codex --full-auto ${quoted_args}"
