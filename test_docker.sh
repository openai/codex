#!/bin/bash

set -euo pipefail

# Usage information
function show_usage {
    echo "Usage: $0 [options]"
    echo ""
    echo "Options:"
    echo "  -h, --help       Show this help message"
    echo "  -k, --keep       Keep the container running after the test (default: stop container)"
    echo "  -q, --quiet      Run in quiet mode with minimal output"
    echo "  -n, --node-info  Show Node.js version information"
    echo ""
    echo "Examples:"
    echo "  $0                  # Run the test and stop container after"
    echo "  $0 --keep           # Run the test and keep container running"
    echo "  $0 --quiet --keep   # Run the test quietly and keep container running"
    echo "  $0 --node-info      # Run the test and show Node.js version info"
    exit 0
}

# Default values
KEEP_RUNNING=false
QUIET_MODE=false
SHOW_NODE_INFO=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)
            show_usage
            ;;
        -k|--keep)
            KEEP_RUNNING=true
            shift
            ;;
        -q|--quiet)
            QUIET_MODE=true
            shift
            ;;
        -n|--node-info)
            SHOW_NODE_INFO=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            show_usage
            ;;
    esac
done

# Output function that respects quiet mode
function log {
    if [[ "$QUIET_MODE" == false ]]; then
        echo "$@"
    fi
}

log "Testing Docker Compose setup for Codex CLI..."

# Check if docker-compose is installed
if ! command -v docker-compose &> /dev/null; then
    echo "Error: docker-compose is not installed. Please install it first."
    exit 1
fi

# Build the container
log "Building Docker container..."
if [[ "$QUIET_MODE" == true ]]; then
    docker-compose build --quiet
else
    docker-compose build
fi

# Start the container in detached mode
log "Starting Docker container..."
docker-compose up -d

# Run a test command in the container
log "Testing Codex CLI inside the container..."
CODEX_VERSION=$(docker-compose exec -T codex-cli bash -c "codex --version")
RESULT=$?

if [ $RESULT -eq 0 ]; then
    log "✅ Docker test successful! Codex CLI is working properly in the container."
    log "Codex version: $CODEX_VERSION"
else
    echo "❌ Docker test failed. Check the error messages above."
    
    # Always stop the container on failure
    log "Stopping Docker container..."
    docker-compose down
    exit 1
fi

# Optionally show Node.js version info
if [[ "$SHOW_NODE_INFO" == true ]]; then
    NODE_VERSION=$(docker-compose exec -T codex-cli bash -c "node --version")
    NODE_INFO=$(docker-compose exec -T codex-cli bash -c "node -e 'console.log(\"Features available:\", { fetch: typeof fetch === \"function\" })'")
    log "Node.js version: $NODE_VERSION"
    log "Node.js info: $NODE_INFO"
fi

# Handle container cleanup based on keep flag
if [[ "$KEEP_RUNNING" == true ]]; then
    log "Container is still running."
    log "You can access it with: docker-compose exec codex-cli bash"
    log "To stop it later, run: docker-compose down"
else
    log "Stopping Docker container..."
    docker-compose down
    log "Container stopped."
fi
