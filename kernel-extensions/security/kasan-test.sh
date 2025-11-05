#!/bin/bash
#
# KASAN (Kernel Address Sanitizer) Test
# Requires KASAN-enabled kernel
#

set -e

echo "🛡️  KASAN Security Test"
echo "====================="

# Check if KASAN is enabled
if ! grep -q CONFIG_KASAN=y /boot/config-$(uname -r) 2>/dev/null; then
    echo "⚠️  KASAN not enabled in current kernel"
    echo "   Rebuild kernel with CONFIG_KASAN=y"
    exit 0
fi

echo "✅ KASAN enabled"
echo ""

# Load modules
echo "Loading AI kernel modules..."
sudo insmod ../linux/ai_scheduler/ai_scheduler.ko || true
sudo insmod ../linux/ai_mem/ai_mem.ko || true
sudo insmod ../linux/ai_gpu/ai_gpu.ko || true

echo ""
echo "Modules loaded. Check dmesg for KASAN reports:"
echo "   sudo dmesg | grep -i kasan"
echo ""

# Wait for potential issues
sleep 5

# Check dmesg for KASAN warnings
if dmesg | tail -100 | grep -q "KASAN"; then
    echo "❌ KASAN detected issues!"
    dmesg | tail -50 | grep -A 10 "KASAN"
    exit 1
else
    echo "✅ No KASAN issues detected"
fi

# Unload modules
echo ""
echo "Unloading modules..."
sudo rmmod ai_gpu || true
sudo rmmod ai_mem || true
sudo rmmod ai_scheduler || true

echo "✅ KASAN test complete"

