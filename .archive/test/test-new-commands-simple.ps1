#!/usr/bin/env pwsh
# Simple Test for New Commands

Write-Host "`n=== Testing New Commands ===" -ForegroundColor Cyan
Write-Host ""

# Test 1: agent-create (simple)
Write-Host "Test 1: agent-create" -ForegroundColor Yellow
Write-Host "Creating a file counter agent..." -ForegroundColor Gray
Write-Host ""

codex agent-create "List all PowerShell script files in the current directory" --budget 3000

Write-Host "`n" + ("=" * 60) -ForegroundColor DarkGray
Write-Host ""

# Test 2: delegate (single, to verify basic functionality)
Write-Host "Test 2: delegate (baseline)" -ForegroundColor Yellow
Write-Host "Testing basic delegate command..." -ForegroundColor Gray
Write-Host ""

codex delegate researcher --goal "Quick test" --budget 2000

Write-Host "`n" + ("=" * 60) -ForegroundColor DarkGray
Write-Host ""

# Test 3: delegate-parallel (single agent as simplest case)
Write-Host "Test 3: delegate-parallel (single agent)" -ForegroundColor Yellow
Write-Host "Testing parallel execution with one agent..." -ForegroundColor Gray
Write-Host ""

codex delegate-parallel researcher --goals "Test parallel mode" --budgets 2000

Write-Host "`n" + ("=" * 60) -ForegroundColor DarkGray
Write-Host ""

Write-Host "=== All Tests Complete ===" -ForegroundColor Green
Write-Host ""

