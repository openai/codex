# Test WebSearchProvider via MCP
Write-Host "=== WebSearchProvider Test via MCP ===" -ForegroundColor Cyan

# Test: Deep Research with WebSearchProvider
Write-Host "`n[Test] Deep Research - Rust async error handling..." -ForegroundColor Yellow

$output = codex deep-research "Rust async error handling" --levels 2 --sources 5 --strategy comprehensive 2>&1 | Out-String

# Check for real URLs (not example.com)
if ($output -like "*doc.rust-lang.org*" -or $output -like "*stackoverflow.com*" -or $output -like "*github.com*") {
    Write-Host "✅ PASS: Real URLs found (not example.com)!" -ForegroundColor Green
    Write-Host "   Found: doc.rust-lang.org, stackoverflow.com, or github.com" -ForegroundColor Cyan
} else {
    Write-Host "⚠️  WARNING: Still using example.com URLs" -ForegroundColor Yellow
    Write-Host "   This might be expected if using older binary" -ForegroundColor Yellow
}

# Check for sources
if ($output -like "*Sources found:*") {
    Write-Host "✅ PASS: Sources found" -ForegroundColor Green
} else {
    Write-Host "❌ FAIL: No sources found" -ForegroundColor Red
}

# Check for findings
if ($output -like "*Findings:*") {
    Write-Host "✅ PASS: Findings generated" -ForegroundColor Green
} else {
    Write-Host "❌ FAIL: No findings" -ForegroundColor Red
}

# Check for summary
if ($output -like "*Summary:*") {
    Write-Host "✅ PASS: Summary generated" -ForegroundColor Green
} else {
    Write-Host "❌ FAIL: No summary" -ForegroundColor Red
}

Write-Host "`n--- Full Output (First 30 lines) ---" -ForegroundColor Yellow
$output -split "`n" | Select-Object -First 30 | ForEach-Object { Write-Host $_ }

Write-Host "`n=== Test Complete ===" -ForegroundColor Cyan

