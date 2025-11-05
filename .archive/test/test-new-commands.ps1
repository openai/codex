# Test New Commands - Phase 4
# Tests delegate-parallel and agent-create functionality

$ErrorActionPreference = "Continue"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Testing Phase 4 New Commands" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if build completed
$binary = "codex-rs\target\release\codex.exe"
if (-not (Test-Path $binary)) {
    Write-Host "ERROR: Binary not found!" -ForegroundColor Red
    Write-Host "Please run: .\fast-build.ps1 -Release" -ForegroundColor Yellow
    exit 1
}

$fileInfo = Get-Item $binary
Write-Host "Binary found:" -ForegroundColor Green
Write-Host "  Size: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor White
Write-Host "  Modified: $($fileInfo.LastWriteTime)" -ForegroundColor White
Write-Host ""

# Install latest build
Write-Host "Installing latest build..." -ForegroundColor Yellow
$installDir = "$env:USERPROFILE\.codex\bin"
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}
Copy-Item -Path $binary -Destination "$installDir\codex.exe" -Force
Write-Host "Installed!" -ForegroundColor Green
Write-Host ""

# Test 1: Check version
Write-Host "[Test 1] Version Check" -ForegroundColor Cyan
codex --version
Write-Host ""

# Test 2: List all commands
Write-Host "[Test 2] Command List" -ForegroundColor Cyan
codex help 2>&1 | Select-Object -First 30
Write-Host ""

# Test 3: delegate-parallel help
Write-Host "[Test 3] delegate-parallel --help" -ForegroundColor Cyan
$delegateHelp = codex delegate-parallel --help 2>&1
if ($delegateHelp -match "delegate-parallel") {
    Write-Host "SUCCESS: delegate-parallel command found!" -ForegroundColor Green
    $delegateHelp | Select-Object -First 20
} else {
    Write-Host "FAILED: delegate-parallel command not recognized" -ForegroundColor Red
    Write-Host "Output:" -ForegroundColor Yellow
    $delegateHelp | Select-Object -First 10
}
Write-Host ""

# Test 4: agent-create help
Write-Host "[Test 4] agent-create --help" -ForegroundColor Cyan
$agentHelp = codex agent-create --help 2>&1
if ($agentHelp -match "agent-create") {
    Write-Host "SUCCESS: agent-create command found!" -ForegroundColor Green
    $agentHelp | Select-Object -First 20
} else {
    Write-Host "FAILED: agent-create command not recognized" -ForegroundColor Red
    Write-Host "Output:" -ForegroundColor Yellow
    $agentHelp | Select-Object -First 10
}
Write-Host ""

# Test 5: Search for new commands in help
Write-Host "[Test 5] Search for new commands" -ForegroundColor Cyan
$allHelp = codex --help 2>&1 | Out-String
if ($allHelp -match "delegate-parallel") {
    Write-Host "  delegate-parallel: FOUND in help" -ForegroundColor Green
} else {
    Write-Host "  delegate-parallel: NOT FOUND in help" -ForegroundColor Red
}

if ($allHelp -match "agent-create") {
    Write-Host "  agent-create: FOUND in help" -ForegroundColor Green
} else {
    Write-Host "  agent-create: NOT FOUND in help" -ForegroundColor Red
}
Write-Host ""

# Test 6: Existing deep-research command
Write-Host "[Test 6] Existing Commands (deep-research)" -ForegroundColor Cyan
$researchHelp = codex deep-research --help 2>&1
if ($researchHelp -match "deep-research") {
    Write-Host "SUCCESS: deep-research still works!" -ForegroundColor Green
} else {
    Write-Host "WARNING: deep-research command issue" -ForegroundColor Yellow
}
Write-Host ""

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Test Summary" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "If delegate-parallel and agent-create are not found," -ForegroundColor Yellow
Write-Host "the build may need to complete fully." -ForegroundColor Yellow
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Wait for build to complete (check with cargo processes)" -ForegroundColor White
Write-Host "  2. Re-run this test: .\test-new-commands.ps1" -ForegroundColor White
Write-Host "  3. Or manually test: codex delegate-parallel --help" -ForegroundColor White
Write-Host ""

