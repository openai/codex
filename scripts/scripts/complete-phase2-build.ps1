# Phase 2 Complete Build and Install Script
# Clean build, release build, and global install

Write-Host "=== Phase 2 Complete Build and Install ===" -ForegroundColor Cyan
Write-Host ""

# Step 1: Navigate to codex-rs
Write-Host "[Step 1/4] Navigating to codex-rs..." -ForegroundColor Yellow
Set-Location -Path "codex-rs"
Write-Host "  Current directory: $(Get-Location)" -ForegroundColor Gray
Write-Host ""

# Step 2: Clean build
Write-Host "[Step 2/4] Clean build..." -ForegroundColor Yellow
cargo clean
Write-Host "  Clean completed" -ForegroundColor Green
Write-Host ""

# Step 3: Release build (with stability options)
Write-Host "[Step 3/4] Release build with stability options (single-threaded, no incremental)..." -ForegroundColor Yellow
Write-Host "  This will take 15-20 minutes but is more stable..." -ForegroundColor Gray
$buildStart = Get-Date
cargo build --release -p codex-cli -j 1
$buildEnd = Get-Date
$buildDuration = ($buildEnd - $buildStart).TotalMinutes

if ($LASTEXITCODE -eq 0) {
    Write-Host "  Build completed in $([math]::Round($buildDuration, 2)) minutes" -ForegroundColor Green
} else {
    Write-Host "  Build failed!" -ForegroundColor Red
    Set-Location -Path ".."
    exit 1
}
Write-Host ""

# Step 4: Global install
Write-Host "[Step 4/4] Global install..." -ForegroundColor Yellow
cargo install --path cli --force

if ($LASTEXITCODE -eq 0) {
    Write-Host "  Install completed" -ForegroundColor Green
} else {
    Write-Host "  Install failed!" -ForegroundColor Red
    Set-Location -Path ".."
    exit 1
}
Write-Host ""

# Return to parent directory
Set-Location -Path ".."

# Verification
Write-Host "=== Verification ===" -ForegroundColor Cyan
Write-Host ""

Write-Host "Codex version:" -ForegroundColor Yellow
codex --version
Write-Host ""

Write-Host "MCP server check:" -ForegroundColor Yellow
codex mcp-server --help | Select-Object -First 3
Write-Host ""

Write-Host "MCP list:" -ForegroundColor Yellow
codex mcp list
Write-Host ""

Write-Host "=== Phase 2 Build Complete! ===" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Test MCP integration: codex delegate code-reviewer --scope ./src" -ForegroundColor White
Write-Host "  2. Use MCP Inspector: npx @modelcontextprotocol/inspector codex mcp-server" -ForegroundColor White
Write-Host "  3. Check audit logs: cat ~/.codex/audit-logs/*.json | jq" -ForegroundColor White
Write-Host ""

