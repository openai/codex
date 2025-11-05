#!/bin/bash
#
# Valgrind Memory Leak Detection
# AI Kernel Modules Security Audit
#

set -e

echo "🔍 Valgrind Security Audit"
echo "=========================="

# Check if valgrind is installed
if ! command -v valgrind &> /dev/null; then
    echo "❌ Valgrind not installed"
    echo "   Install with: sudo apt install valgrind"
    exit 1
fi

# Test programs
TEST_PROGRAMS=(
    "kernel-stats"
    "ai-monitor"
)

# Valgrind options
VALGRIND_OPTS=(
    "--leak-check=full"
    "--show-leak-kinds=all"
    "--track-origins=yes"
    "--verbose"
    "--log-file=valgrind-%p.log"
)

echo ""
echo "📋 Running memory leak detection..."
echo ""

for program in "${TEST_PROGRAMS[@]}"; do
    if [ -f "../codex-integration/target/release/$program" ]; then
        echo "Testing: $program"
        valgrind "${VALGRIND_OPTS[@]}" \
            "../codex-integration/target/release/$program" || true
        echo ""
    fi
done

echo "✅ Valgrind audit complete"
echo "   Check valgrind-*.log for details"

