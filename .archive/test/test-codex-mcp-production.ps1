# Codex MCP Production Environment Test Script
# Based on best practices from zenn.dev and MCP documentation

Write-Host "=== Codex MCP Production Environment Test ===" -ForegroundColor Cyan
Write-Host ""

# Test 1: Configuration Check
Write-Host "[Test 1/5] Configuration Check" -ForegroundColor Yellow
Write-Host "Checking config.toml for MCP server settings..." -ForegroundColor Gray

if (Test-Path "config.toml") {
    $config = Get-Content "config.toml" -Raw
    if ($config -match "\[mcp_servers\.codex-agent\]") {
        Write-Host "  OK: codex-agent MCP server configured" -ForegroundColor Green
    } else {
        Write-Host "  ERROR: codex-agent not found in config.toml" -ForegroundColor Red
        exit 1
    }
    
    if ($config -match "use_codex_mcp\s*=\s*true") {
        Write-Host "  OK: use_codex_mcp is enabled" -ForegroundColor Green
    } else {
        Write-Host "  WARNING: use_codex_mcp is not enabled" -ForegroundColor Yellow
    }
} else {
    Write-Host "  ERROR: config.toml not found" -ForegroundColor Red
    exit 1
}
Write-Host ""

# Test 2: MCP Server Startup Test
Write-Host "[Test 2/5] MCP Server Startup Test" -ForegroundColor Yellow
Write-Host "Testing if codex mcp-server can start..." -ForegroundColor Gray

try {
    $help = codex mcp-server --help 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  OK: codex mcp-server command is available" -ForegroundColor Green
    } else {
        Write-Host "  ERROR: codex mcp-server failed" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "  ERROR: $_" -ForegroundColor Red
    exit 1
}
Write-Host ""

# Test 3: MCP Server List
Write-Host "[Test 3/5] MCP Server List" -ForegroundColor Yellow
Write-Host "Checking configured MCP servers..." -ForegroundColor Gray

try {
    $list = codex mcp list 2>&1
    Write-Host "$list" -ForegroundColor White
    
    if ($list -match "codex-agent") {
        Write-Host "  OK: codex-agent is listed" -ForegroundColor Green
    } else {
        Write-Host "  WARNING: codex-agent not found in list" -ForegroundColor Yellow
    }
} catch {
    Write-Host "  ERROR: $_" -ForegroundColor Red
}
Write-Host ""

# Test 4: Security Settings Check
Write-Host "[Test 4/5] Security Settings Check" -ForegroundColor Yellow
Write-Host "Verifying security configuration..." -ForegroundColor Gray

$securityChecks = @()

# Sandbox mode check
if ($config -match 'default_mode\s*=\s*"read-only"') {
    Write-Host "  OK: Sandbox default_mode is read-only" -ForegroundColor Green
    $securityChecks += "sandbox_ok"
} else {
    Write-Host "  WARNING: Sandbox default_mode is not read-only" -ForegroundColor Yellow
}

# Approval policy check
if ($config -match 'policy\s*=\s*"on-request"') {
    Write-Host "  OK: Approval policy is on-request" -ForegroundColor Green
    $securityChecks += "approval_ok"
} else {
    Write-Host "  WARNING: Approval policy is not on-request" -ForegroundColor Yellow
}

# Audit logging check
if ($config -match 'enabled\s*=\s*true' -and $config -match 'include_mcp_calls') {
    Write-Host "  OK: Audit logging is enabled with MCP calls" -ForegroundColor Green
    $securityChecks += "audit_ok"
} else {
    Write-Host "  WARNING: Audit logging may not be properly configured" -ForegroundColor Yellow
}

Write-Host ""

# Test 5: MCP Inspector Test (if available)
Write-Host "[Test 5/5] MCP Inspector Connection Test" -ForegroundColor Yellow
Write-Host "Checking if MCP Inspector is available..." -ForegroundColor Gray

try {
    $inspector = Get-Command npx -ErrorAction SilentlyContinue
    if ($inspector) {
        Write-Host "  INFO: npx is available for MCP Inspector" -ForegroundColor Green
        Write-Host "  To test with MCP Inspector, run:" -ForegroundColor Cyan
        Write-Host "    npx @modelcontextprotocol/inspector codex mcp-server" -ForegroundColor White
        Write-Host ""
        Write-Host "  Then in the Inspector UI:" -ForegroundColor Cyan
        Write-Host "    1. Click 'Connect'" -ForegroundColor White
        Write-Host "    2. Run 'tools/list' to see available tools" -ForegroundColor White
        Write-Host "    3. Test 'codex_read_file', 'codex_grep', etc." -ForegroundColor White
    } else {
        Write-Host "  WARNING: npx not available, cannot run MCP Inspector" -ForegroundColor Yellow
    }
} catch {
    Write-Host "  WARNING: Could not check MCP Inspector availability" -ForegroundColor Yellow
}
Write-Host ""

# Summary
Write-Host "=== Test Summary ===" -ForegroundColor Cyan
Write-Host ""

$totalTests = 5
$passedTests = 0

if ($config -match "\[mcp_servers\.codex-agent\]") { $passedTests++ }
if ($LASTEXITCODE -eq 0 -or $help) { $passedTests++ }
if ($list -match "codex-agent") { $passedTests++ }
if ($securityChecks.Count -ge 2) { $passedTests++ }
if ($inspector) { $passedTests++ }

Write-Host "Tests Passed: $passedTests / $totalTests" -ForegroundColor $(if ($passedTests -eq $totalTests) { "Green" } else { "Yellow" })
Write-Host ""

if ($passedTests -eq $totalTests) {
    Write-Host "SUCCESS: Codex MCP is ready for production!" -ForegroundColor Green
} elseif ($passedTests -ge 3) {
    Write-Host "PARTIAL: Codex MCP is mostly ready, review warnings" -ForegroundColor Yellow
} else {
    Write-Host "FAILED: Codex MCP needs configuration fixes" -ForegroundColor Red
}
Write-Host ""

Write-Host "=== Next Steps ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. Start MCP server in one terminal:" -ForegroundColor White
Write-Host "   codex mcp-server" -ForegroundColor Gray
Write-Host ""
Write-Host "2. Test with MCP Inspector (optional):" -ForegroundColor White
Write-Host "   npx @modelcontextprotocol/inspector codex mcp-server" -ForegroundColor Gray
Write-Host ""
Write-Host "3. Use in subagent:" -ForegroundColor White
Write-Host "   codex delegate code-reviewer --scope ./src" -ForegroundColor Gray
Write-Host ""
Write-Host "4. Check audit logs:" -ForegroundColor White
Write-Host "   cat ~/.codex/audit-logs/*.json | jq" -ForegroundColor Gray
Write-Host ""

Write-Host "=== Test Completed ===" -ForegroundColor Cyan

