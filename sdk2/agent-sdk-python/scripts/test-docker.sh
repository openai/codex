#!/bin/bash
# Run SDK tests in a Docker container
# This helps catch Docker-specific issues like #406
#
# Usage:
#   ./scripts/test-docker.sh [unit|e2e|all]
#
# Examples:
#   ./scripts/test-docker.sh unit                    # Run unit tests only
#   ANTHROPIC_API_KEY=sk-... ./scripts/test-docker.sh e2e   # Run e2e tests
#   ANTHROPIC_API_KEY=sk-... ./scripts/test-docker.sh all   # Run all tests

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

usage() {
    echo "Usage: $0 [unit|e2e|all]"
    echo ""
    echo "Commands:"
    echo "  unit  - Run unit tests only (no API key needed)"
    echo "  e2e   - Run e2e tests (requires ANTHROPIC_API_KEY)"
    echo "  all   - Run both unit and e2e tests"
    echo ""
    echo "Examples:"
    echo "  $0 unit"
    echo "  ANTHROPIC_API_KEY=sk-... $0 e2e"
    exit 1
}

echo "Building Docker test image..."
docker build -f Dockerfile.test -t codex-sdk-test .

case "${1:-unit}" in
    unit)
        echo ""
        echo "Running unit tests in Docker..."
        docker run --rm codex-sdk-test \
            python -m pytest tests/ -v
        ;;
    e2e)
        if [ -z "$ANTHROPIC_API_KEY" ]; then
            echo "Error: ANTHROPIC_API_KEY environment variable is required for e2e tests"
            echo ""
            echo "Usage: ANTHROPIC_API_KEY=sk-... $0 e2e"
            exit 1
        fi
        echo ""
        echo "Running e2e tests in Docker..."
        docker run --rm -e ANTHROPIC_API_KEY \
            codex-sdk-test python -m pytest e2e-tests/ -v -m e2e
        ;;
    all)
        echo ""
        echo "Running unit tests in Docker..."
        docker run --rm codex-sdk-test \
            python -m pytest tests/ -v

        echo ""
        if [ -n "$ANTHROPIC_API_KEY" ]; then
            echo "Running e2e tests in Docker..."
            docker run --rm -e ANTHROPIC_API_KEY \
                codex-sdk-test python -m pytest e2e-tests/ -v -m e2e
        else
            echo "Skipping e2e tests (ANTHROPIC_API_KEY not set)"
        fi
        ;;
    *)
        usage
        ;;
esac

echo ""
echo "Done!"
