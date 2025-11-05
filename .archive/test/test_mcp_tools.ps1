# MCP Tools Test Script
Write-Host "=== MCP Server Tools Test ===" -ForegroundColor Cyan

# Step 1: Initialize
Write-Host "`n[Step 1] Initializing MCP Server..." -ForegroundColor Yellow
$init = @'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}
'@

# Step 2: List Tools
Write-Host "[Step 2] Listing available tools..." -ForegroundColor Yellow
$listTools = @'
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
'@

# Create temp input file
$inputFile = "mcp_test_input.txt"
@"
$init
$listTools
"@ | Out-File -FilePath $inputFile -Encoding UTF8

# Run MCP server
Write-Host "[Step 3] Running MCP Server..." -ForegroundColor Yellow
$output = Get-Content $inputFile | codex mcp-server 2>&1

# Parse output
Write-Host "`n=== MCP Server Output ===" -ForegroundColor Cyan
$output | ForEach-Object {
    if ($_ -like '*"tools"*') {
        Write-Host "Found tools list!" -ForegroundColor Green
        $_ | ConvertFrom-Json | ConvertTo-Json -Depth 10 | Write-Host
    }
}

# Cleanup
Remove-Item $inputFile -ErrorAction SilentlyContinue

Write-Host "`n=== Test Complete ===" -ForegroundColor Cyan

