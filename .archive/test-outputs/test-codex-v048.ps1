# Codex v0.48.0 Real Device Test
# 2025-10-16

Write-Host "Codex v0.48.0 Real Device Test" -ForegroundColor Cyan
Write-Host "================================" -ForegroundColor Gray
Write-Host ""

$passCount = 0
$failCount = 0

# Test 1: Version Check
Write-Host "[Test 1/8] Version Check" -ForegroundColor Yellow
try {
    $version = & codex --version 2>&1 | Out-String
    if ($version -match "0.48.0") {
        Write-Host "  Result: codex-cli 0.48.0" -ForegroundColor Green
        Write-Host "  PASS" -ForegroundColor Green
        $passCount++
    } else {
        Write-Host "  FAIL: Unexpected version" -ForegroundColor Red
        $failCount++
    }
} catch {
    Write-Host "  FAIL: $_" -ForegroundColor Red
    $failCount++
}
Write-Host ""

# Test 2: Help Display
Write-Host "[Test 2/8] Help Display" -ForegroundColor Yellow
try {
    $help = & codex --help 2>&1 | Out-String
    if ($help -match "agent" -and $help -match "exec" -and $help -match "resume") {
        Write-Host "  Main subcommands detected: agent, exec, resume" -ForegroundColor Green
        Write-Host "  PASS" -ForegroundColor Green
        $passCount++
    } else {
        Write-Host "  FAIL: Main subcommands not found" -ForegroundColor Red
        $failCount++
    }
} catch {
    Write-Host "  FAIL: $_" -ForegroundColor Red
    $failCount++
}
Write-Host ""

# Test 3: Agent Subcommand
Write-Host "[Test 3/8] Agent Subcommand" -ForegroundColor Yellow
try {
    $agentHelp = & codex agent --help 2>&1 | Out-String
    if ($agentHelp -match "Natural language") {
        Write-Host "  Agent feature confirmed" -ForegroundColor Green
        Write-Host "  PASS" -ForegroundColor Green
        $passCount++
    } else {
        Write-Host "  FAIL: Agent feature not found" -ForegroundColor Red
        $failCount++
    }
} catch {
    Write-Host "  FAIL: $_" -ForegroundColor Red
    $failCount++
}
Write-Host ""

# Test 4: Exec Subcommand
Write-Host "[Test 4/8] Exec Subcommand" -ForegroundColor Yellow
try {
    $execHelp = & codex exec --help 2>&1 | Out-String
    if ($execHelp -match "non-interactively") {
        Write-Host "  Exec feature confirmed" -ForegroundColor Green
        Write-Host "  PASS" -ForegroundColor Green
        $passCount++
    } else {
        Write-Host "  FAIL: Exec feature not found" -ForegroundColor Red
        $failCount++
    }
} catch {
    Write-Host "  FAIL: $_" -ForegroundColor Red
    $failCount++
}
Write-Host ""

# Test 5: Binary Existence Check
Write-Host "[Test 5/8] Binary Existence Check" -ForegroundColor Yellow
$codexPath = "$env:USERPROFILE\.cargo\bin\codex.exe"
if (Test-Path $codexPath) {
    $bin = Get-Item $codexPath
    $sizeMB = [math]::Round($bin.Length/1MB, 2)
    Write-Host "  Path: $codexPath" -ForegroundColor White
    Write-Host "  Size: $sizeMB MB" -ForegroundColor White
    Write-Host "  Modified: $($bin.LastWriteTime)" -ForegroundColor White
    Write-Host "  PASS" -ForegroundColor Green
    $passCount++
} else {
    Write-Host "  FAIL: codex.exe not found" -ForegroundColor Red
    $failCount++
}
Write-Host ""

# Test 6: PATH Environment Variable Check
Write-Host "[Test 6/8] PATH Environment Variable Check" -ForegroundColor Yellow
$cargoPath = "$env:USERPROFILE\.cargo\bin"
if ($env:Path -like "*$cargoPath*") {
    Write-Host "  Cargo bin directory in PATH: OK" -ForegroundColor Green
    Write-Host "  PASS" -ForegroundColor Green
    $passCount++
} else {
    Write-Host "  FAIL: Cargo bin directory not in PATH" -ForegroundColor Yellow
    $failCount++
}
Write-Host ""

# Test 7: ThreeWayMerge Implementation Check
Write-Host "[Test 7/8] ThreeWayMerge Implementation Check" -ForegroundColor Yellow
$conflictResolverPath = "codex-rs\core\src\orchestration\conflict_resolver.rs"
if (Test-Path $conflictResolverPath) {
    $content = Get-Content $conflictResolverPath -Raw
    if ($content -match "resolve_three_way") {
        Write-Host "  resolve_three_way function: Implemented" -ForegroundColor Green
        Write-Host "  PASS" -ForegroundColor Green
        $passCount++
    } else {
        Write-Host "  FAIL: resolve_three_way function not found" -ForegroundColor Yellow
        $failCount++
    }
} else {
    Write-Host "  FAIL: conflict_resolver.rs not found" -ForegroundColor Red
    $failCount++
}
Write-Host ""

# Test 8: Delegate Command Check
Write-Host "[Test 8/8] Delegate Command Check" -ForegroundColor Yellow
try {
    $delegateHelp = & codex delegate --help 2>&1 | Out-String
    if ($delegateHelp -match "sub-agent") {
        Write-Host "  Delegate feature confirmed" -ForegroundColor Green
        Write-Host "  PASS" -ForegroundColor Green
        $passCount++
    } else {
        Write-Host "  FAIL: Delegate feature not found" -ForegroundColor Red
        $failCount++
    }
} catch {
    Write-Host "  FAIL: $_" -ForegroundColor Red
    $failCount++
}
Write-Host ""

Write-Host "================================" -ForegroundColor Gray
Write-Host "Test Summary" -ForegroundColor Cyan
Write-Host "  PASS: $passCount / 8" -ForegroundColor Green
Write-Host "  FAIL: $failCount / 8" -ForegroundColor $(if ($failCount -eq 0) { "Green" } else { "Red" })
Write-Host ""

if ($failCount -eq 0) {
    Write-Host "All tests passed!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "Some tests failed." -ForegroundColor Yellow
    exit 1
}
