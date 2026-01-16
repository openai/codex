#!/usr/bin/env bash
# Smart build script that detects stale cache issues and auto-cleans affected packages
#
# Usage: ./scripts/smart-build.sh [cargo build args...]
#
# This script wraps `cargo build` and automatically detects common cache-related
# compilation errors (like "no field...on type" errors that occur when struct
# definitions change but cargo's incremental cache becomes stale).
#
# When such errors are detected, it automatically runs `cargo clean -p <crate>`
# on the affected crate and retries the build.

set -e

# Colors for output
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

# Default to building codex binary if no args provided
BUILD_ARGS="${@:-build --bin codex}"

# Attempt initial build
echo -e "${GREEN}Running: cargo ${BUILD_ARGS}${NC}"
BUILD_OUTPUT=$(cargo ${BUILD_ARGS} 2>&1) || BUILD_EXIT=$?

# If build succeeded, we're done
if [ -z "${BUILD_EXIT}" ]; then
    echo -e "${GREEN}Build succeeded!${NC}"
    exit 0
fi

# Check if error is about missing fields (stale cache indicator)
if echo "$BUILD_OUTPUT" | grep -q "no field.*on type"; then
    echo -e "${YELLOW}Detected stale cache issue (missing fields on type)${NC}"
    
    # Extract the affected type/crate from error message
    # Example: "no field `spec` on type `codex_tui::Cli`" -> codex_tui
    AFFECTED_CRATE=$(echo "$BUILD_OUTPUT" | grep "no field.*on type" | head -1 | sed -n 's/.*on type `\([^:]*\)::.*/\1/p')
    
    if [ -n "$AFFECTED_CRATE" ]; then
        # Convert crate name from snake_case to kebab-case for cargo clean
        AFFECTED_CRATE_KEBAB=$(echo "$AFFECTED_CRATE" | tr '_' '-')
        
        echo -e "${YELLOW}Cleaning affected crate: ${AFFECTED_CRATE_KEBAB}${NC}"
        cargo clean -p "$AFFECTED_CRATE_KEBAB"
        
        echo -e "${GREEN}Retrying build after cleanup...${NC}"
        cargo ${BUILD_ARGS}
        exit $?
    else
        echo -e "${YELLOW}Could not identify affected crate, cleaning all workspace caches${NC}"
        cargo clean
        cargo ${BUILD_ARGS}
        exit $?
    fi
fi

# If it's a different error, show output and exit with error
echo -e "${RED}Build failed with error:${NC}"
echo "$BUILD_OUTPUT"
exit ${BUILD_EXIT}
