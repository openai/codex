# MCP Server Test Script
Write-Host "=== MCP Server Test ===" -ForegroundColor Cyan

# Test 1: Initialize
Write-Host "`n[Test 1] Initializing MCP Server..." -ForegroundColor Yellow
$init = '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
$result1 = $init | codex mcp-server 2>&1 | Select-Object -First 1
Write-Host "Result: $result1" -ForegroundColor Green

# Parse and check if successful
if ($result1 -like '*"result"*') {
    Write-Host "✅ Initialize: PASS" -ForegroundColor Green
} else {
    Write-Host "❌ Initialize: FAIL" -ForegroundColor Red
}

Write-Host "`n=== Test Complete ===" -ForegroundColor Cyan

