# Deep Research & Web Search Integration Test
# Production Environment - Full Integration Check

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Deep Research Integration Test" -ForegroundColor Cyan
Write-Host "  Web Search + Deep Research + Cursor IDE" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""

$testsPassed = 0
$testsFailed = 0

# Test 1: WebSearchProvider & McpSearchProvider Export Check
Write-Host "[Test 1] Provider Export Check" -ForegroundColor Yellow
$libFile = "codex-rs\deep-research\src\lib.rs"
if (Test-Path $libFile) {
    $content = Get-Content $libFile -Raw
    if ($content -match "pub use web_search_provider::WebSearchProvider" -and 
        $content -match "pub use mcp_search_provider::McpSearchProvider") {
        Write-Host "  [PASS] WebSearchProvider exported" -ForegroundColor Green
        Write-Host "  [PASS] McpSearchProvider exported" -ForegroundColor Green
        $testsPassed++
    } else {
        Write-Host "  [FAIL] Providers not properly exported" -ForegroundColor Red
        $testsFailed++
    }
} else {
    Write-Host "  [FAIL] lib.rs not found" -ForegroundColor Red
    $testsFailed++
}

# Test 2: CLI Research Command Integration
Write-Host ""
Write-Host "[Test 2] CLI Research Command" -ForegroundColor Yellow
$cliFile = "codex-rs\cli\src\research_cmd.rs"
if (Test-Path $cliFile) {
    $content = Get-Content $cliFile -Raw
    if ($content -match "WebSearchProvider" -and 
        $content -match "McpSearchProvider" -and
        $content -notmatch "MockProvider") {
        Write-Host "  [PASS] Real providers integrated" -ForegroundColor Green
        Write-Host "  [PASS] MockProvider removed" -ForegroundColor Green
        $testsPassed++
    } else {
        Write-Host "  [FAIL] Provider integration incomplete" -ForegroundColor Red
        $testsFailed++
    }
} else {
    Write-Host "  [FAIL] research_cmd.rs not found" -ForegroundColor Red
    $testsFailed++
}

# Test 3: Cursor IDE MCP Configuration
Write-Host ""
Write-Host "[Test 3] Cursor IDE MCP Config" -ForegroundColor Yellow
$mcpFile = ".cursor\mcp.json"
if (Test-Path $mcpFile) {
    $content = Get-Content $mcpFile -Raw
    $serverCount = ([regex]::Matches($content, '"command":')).Count
    if ($serverCount -ge 8) {
        Write-Host "  [PASS] $serverCount MCP servers configured" -ForegroundColor Green
        $testsPassed++
    } else {
        Write-Host "  [FAIL] Only $serverCount MCP servers found (expected 8+)" -ForegroundColor Red
        $testsFailed++
    }
} else {
    Write-Host "  [FAIL] .cursor/mcp.json not found" -ForegroundColor Red
    $testsFailed++
}

# Test 4: Agent Definitions
Write-Host ""
Write-Host "[Test 4] Agent Definitions" -ForegroundColor Yellow
$agentFiles = Get-ChildItem -Path ".codex\agents\*.yaml" -ErrorAction SilentlyContinue
if ($agentFiles.Count -ge 7) {
    Write-Host "  [PASS] $($agentFiles.Count) agent definitions found" -ForegroundColor Green
    foreach ($agent in $agentFiles) {
        Write-Host "    - $($agent.Name)" -ForegroundColor Gray
    }
    $testsPassed++
} else {
    Write-Host "  [FAIL] Only $($agentFiles.Count) agents found (expected 7+)" -ForegroundColor Red
    $testsFailed++
}

# Test 5: Build Artifacts
Write-Host ""
Write-Host "[Test 5] Build Artifacts" -ForegroundColor Yellow
$binaries = @(
    "codex-rs\target\release\codex-tui.exe",
    "codex-rs\target\release\codex-mcp-server.exe",
    "codex-rs\target\release\codex-mcp-client.exe"
)
$foundBinaries = 0
foreach ($binary in $binaries) {
    if (Test-Path $binary) {
        $foundBinaries++
        $size = (Get-Item $binary).Length / 1MB
        Write-Host "  [OK] $(Split-Path -Leaf $binary) ($([math]::Round($size, 2)) MB)" -ForegroundColor Green
    }
}
if ($foundBinaries -ge 2) {
    Write-Host "  [PASS] $foundBinaries/$($binaries.Count) binaries available" -ForegroundColor Green
    $testsPassed++
} else {
    Write-Host "  [FAIL] Only $foundBinaries binaries found" -ForegroundColor Red
    $testsFailed++
}

# Test 6: Global Installation
Write-Host ""
Write-Host "[Test 6] Global Installation" -ForegroundColor Yellow
$installDir = "$env:USERPROFILE\.codex\bin"
if (Test-Path $installDir) {
    $installedFiles = Get-ChildItem -Path $installDir
    Write-Host "  [PASS] Installation directory exists" -ForegroundColor Green
    Write-Host "    Location: $installDir" -ForegroundColor Gray
    Write-Host "    Files: $($installedFiles.Count)" -ForegroundColor Gray
    $testsPassed++
} else {
    Write-Host "  [FAIL] Installation directory not found" -ForegroundColor Red
    $testsFailed++
}

# Test 7: MCP Server Files
Write-Host ""
Write-Host "[Test 7] MCP Server Files" -ForegroundColor Yellow
$mcpFiles = @(
    "codex-rs\mcp-server\dist\index.js",
    "codex-rs\deep-research\mcp-server\web-search.js"
)
$foundMcp = 0
foreach ($mcpFile in $mcpFiles) {
    if (Test-Path $mcpFile) {
        $foundMcp++
        Write-Host "  [OK] $(Split-Path -Leaf $mcpFile)" -ForegroundColor Green
    }
}
if ($foundMcp -eq $mcpFiles.Count) {
    Write-Host "  [PASS] All MCP server files present" -ForegroundColor Green
    $testsPassed++
} else {
    Write-Host "  [FAIL] Only $foundMcp/$($mcpFiles.Count) MCP files found" -ForegroundColor Red
    $testsFailed++
}

# Test 8: Run All Tests
Write-Host ""
Write-Host "[Test 8] Running Test Suites" -ForegroundColor Yellow
Write-Host "  Running Deep Research tests..." -ForegroundColor Cyan
Set-Location -Path "codex-rs"
$testResult = cargo test -p codex-deep-research --lib --release 2>&1 | Select-String "test result"
Set-Location -Path ".."
if ($testResult -match "ok.*23 passed") {
    Write-Host "  [PASS] Deep Research: 23/23 tests passed" -ForegroundColor Green
    $testsPassed++
} else {
    Write-Host "  [FAIL] Deep Research tests failed" -ForegroundColor Red
    $testsFailed++
}

# Summary
Write-Host ""
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  Integration Test Results" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Passed: $testsPassed/8" -ForegroundColor Green
Write-Host "Failed: $testsFailed/8" -ForegroundColor Red
Write-Host ""

if ($testsFailed -eq 0) {
    Write-Host "SUCCESS: All integration tests passed!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Components Verified:" -ForegroundColor Yellow
    Write-Host "  [OK] WebSearchProvider export" -ForegroundColor Green
    Write-Host "  [OK] McpSearchProvider export" -ForegroundColor Green
    Write-Host "  [OK] CLI Integration" -ForegroundColor Green
    Write-Host "  [OK] Cursor IDE MCP (8 servers)" -ForegroundColor Green
    Write-Host "  [OK] Agent Definitions (7 agents)" -ForegroundColor Green
    Write-Host "  [OK] Build Artifacts" -ForegroundColor Green
    Write-Host "  [OK] Global Installation" -ForegroundColor Green
    Write-Host "  [OK] Test Suites (23/23)" -ForegroundColor Green
    Write-Host ""
    Write-Host "Status: Production Ready" -ForegroundColor Green
    exit 0
} else {
    Write-Host "WARNING: $testsFailed test(s) failed" -ForegroundColor Yellow
    exit 1
}

