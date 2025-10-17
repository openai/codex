#!/usr/bin/env pwsh
# Test Parallel Execution and Custom Agent Creation

Write-Host "`n=== Testing New Features ===" -ForegroundColor Cyan
Write-Host ""

# Test 1: agent-create
Write-Host "Test 1: Custom Agent Creation" -ForegroundColor Yellow
Write-Host "Creating a simple file counter agent..." -ForegroundColor Gray
Write-Host ""

codex agent-create "Count all .rs files in the current directory and summarize by subdirectory" --budget 5000 2>&1 | Tee-Object -FilePath "test-agent-create.log"

Write-Host "`n" + ("=" * 60) -ForegroundColor DarkGray
Write-Host ""

# Test 2: delegate-parallel (simplified test with researcher only)
Write-Host "Test 2: Parallel Agent Execution" -ForegroundColor Yellow
Write-Host "Running researcher agent in parallel mode (simplified)..." -ForegroundColor Gray
Write-Host ""

# Single agent parallel test first
codex delegate-parallel researcher --goals "Find Rust async best practices" --budgets 10000 2>&1 | Tee-Object -FilePath "test-parallel-single.log"

Write-Host "`n" + ("=" * 60) -ForegroundColor DarkGray
Write-Host ""

# Test 3: Multiple agents in parallel (if Test 2 succeeded)
Write-Host "Test 3: Multiple Agents in Parallel (Advanced)" -ForegroundColor Yellow
Write-Host "Running 2 researchers with different topics..." -ForegroundColor Gray
Write-Host ""

codex delegate-parallel researcher,researcher --goals "Rust error handling patterns,Rust testing strategies" --budgets 8000,8000 2>&1 | Tee-Object -FilePath "test-parallel-multiple.log"

Write-Host "`n" + ("=" * 60) -ForegroundColor DarkGray
Write-Host ""

# Summary
Write-Host "=== Test Summary ===" -ForegroundColor Cyan
Write-Host ""

$test1Success = Test-Path "test-agent-create.log"
$test2Success = Test-Path "test-parallel-single.log"
$test3Success = Test-Path "test-parallel-multiple.log"

if ($test1Success) {
    Write-Host "[OK] Test 1: agent-create log generated" -ForegroundColor Green
} else {
    Write-Host "[FAIL] Test 1: No log generated" -ForegroundColor Red
}

if ($test2Success) {
    Write-Host "[OK] Test 2: parallel (single) log generated" -ForegroundColor Green
} else {
    Write-Host "[FAIL] Test 2: No log generated" -ForegroundColor Red
}

if ($test3Success) {
    Write-Host "[OK] Test 3: parallel (multiple) log generated" -ForegroundColor Green
} else {
    Write-Host "[FAIL] Test 3: No log generated" -ForegroundColor Red
}

Write-Host ""
Write-Host "Logs saved to:" -ForegroundColor Yellow
Write-Host "  - test-agent-create.log"
Write-Host "  - test-parallel-single.log"
Write-Host "  - test-parallel-multiple.log"
Write-Host ""

