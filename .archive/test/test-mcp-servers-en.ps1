# MCP Servers Test Script
# Test if each MCP server can start correctly

Write-Host "=== MCP Servers Test Started ===" -ForegroundColor Cyan
Write-Host ""

# Test results
$results = @()

# 1. Codex MCP Server (codex-agent)
Write-Host "[1/9] Testing codex-agent..." -ForegroundColor Yellow
try {
    $output = codex mcp-server --help 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  OK: codex-agent" -ForegroundColor Green
        $results += [PSCustomObject]@{Server="codex-agent"; Status="OK"; Command="codex mcp-server"}
    } else {
        Write-Host "  FAILED: codex-agent" -ForegroundColor Red
        $results += [PSCustomObject]@{Server="codex-agent"; Status="FAILED"; Command="codex mcp-server"}
    }
} catch {
    Write-Host "  ERROR: codex-agent - $_" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="codex-agent"; Status="ERROR"; Command="codex mcp-server"}
}
Write-Host ""

# 2. Codex (external)
Write-Host "[2/9] Testing codex (external)..." -ForegroundColor Yellow
try {
    $help = codex mcp --help 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  OK: codex (external)" -ForegroundColor Green
        $results += [PSCustomObject]@{Server="codex"; Status="OK"; Command="codex mcp"}
    } else {
        Write-Host "  NOT FOUND: codex mcp command" -ForegroundColor Red
        $results += [PSCustomObject]@{Server="codex"; Status="NOT FOUND"; Command="codex mcp"}
    }
} catch {
    Write-Host "  ERROR: codex (external)" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="codex"; Status="ERROR"; Command="codex mcp"}
}
Write-Host ""

# 3. Node.js / npx check
Write-Host "[3/9] Checking Node.js and npx..." -ForegroundColor Yellow
try {
    $node = node --version 2>&1
    $npx = npx --version 2>&1
    Write-Host "  Node.js: $node" -ForegroundColor Green
    Write-Host "  npx: $npx" -ForegroundColor Green
    $results += [PSCustomObject]@{Server="node/npx"; Status="OK"; Command="node/npx"}
} catch {
    Write-Host "  ERROR: Node.js or npx not found" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="node/npx"; Status="ERROR"; Command="node/npx"}
}
Write-Host ""

# 4. Python / uv check
Write-Host "[4/9] Checking Python and uv..." -ForegroundColor Yellow
try {
    $python = py -3 --version 2>&1
    Write-Host "  Python: $python" -ForegroundColor Green
    
    $uv = Get-Command uv -ErrorAction SilentlyContinue
    if ($uv) {
        $uvVersion = uv --version 2>&1
        Write-Host "  uv: $uvVersion" -ForegroundColor Green
        $results += [PSCustomObject]@{Server="python/uv"; Status="OK"; Command="python/uv"}
    } else {
        Write-Host "  WARNING: uv not found (needed for markitdown, arxiv)" -ForegroundColor Yellow
        $results += [PSCustomObject]@{Server="python/uv"; Status="uv missing"; Command="python/uv"}
    }
} catch {
    Write-Host "  ERROR: Python or uv check failed" -ForegroundColor Red
    $results += [PSCustomObject]@{Server="python/uv"; Status="ERROR"; Command="python/uv"}
}
Write-Host ""

# 5-9. NPX-based servers (skip actual execution, just note availability)
Write-Host "[5/9] playwright - npx based (will download on first use)" -ForegroundColor Gray
$results += [PSCustomObject]@{Server="playwright"; Status="READY"; Command="npx -y @playwright/mcp@latest"}

Write-Host "[6/9] context7 - npx based (will download on first use)" -ForegroundColor Gray
$results += [PSCustomObject]@{Server="context7"; Status="READY"; Command="npx -y @upstash/context7-mcp"}

Write-Host "[7/9] youtube - npx based (will download on first use)" -ForegroundColor Gray
$results += [PSCustomObject]@{Server="youtube"; Status="READY"; Command="npx @anaisbetts/mcp-youtube"}

Write-Host "[8/9] gemini-cli - npx based (will download on first use)" -ForegroundColor Gray
$results += [PSCustomObject]@{Server="gemini-cli"; Status="READY"; Command="npx mcp-gemini-cli"}

Write-Host "[9/9] chrome-devtools - npx based (will download on first use)" -ForegroundColor Gray
$results += [PSCustomObject]@{Server="chrome-devtools"; Status="READY"; Command="npx chrome-devtools-mcp@latest"}

Write-Host ""

# Results summary
Write-Host "=== Test Results Summary ===" -ForegroundColor Cyan
Write-Host ""
$results | Format-Table -AutoSize
Write-Host ""

# Statistics
$ok = ($results | Where-Object {$_.Status -eq "OK"}).Count
$ready = ($results | Where-Object {$_.Status -eq "READY"}).Count
$failed = ($results | Where-Object {$_.Status -like "*ERROR*" -or $_.Status -like "*FAILED*" -or $_.Status -like "*missing*" -or $_.Status -eq "NOT FOUND"}).Count

Write-Host "Statistics:" -ForegroundColor Cyan
Write-Host "  OK: $ok" -ForegroundColor Green
Write-Host "  READY: $ready" -ForegroundColor Green
Write-Host "  FAILED: $failed" -ForegroundColor Red
Write-Host ""

# Recommendations
Write-Host "=== Recommendations ===" -ForegroundColor Cyan
Write-Host ""

$uvMissing = $results | Where-Object {$_.Status -like "*uv missing*"}
if ($uvMissing) {
    Write-Host "WARNING: uv is not installed. To use markitdown and arxiv servers:" -ForegroundColor Yellow
    Write-Host "  pip install uv" -ForegroundColor White
    Write-Host "  or" -ForegroundColor Gray
    Write-Host "  pipx install uv" -ForegroundColor White
    Write-Host ""
}

Write-Host "Note: npx-based servers will auto-download on first use" -ForegroundColor Gray
Write-Host ""

Write-Host "=== Test Completed ===" -ForegroundColor Cyan

