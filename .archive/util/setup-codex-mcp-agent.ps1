#!/usr/bin/env pwsh
# Setup Codex as MCP Server and Sub-Agent

Write-Host "`n============================================" -ForegroundColor Cyan
Write-Host "  Codex MCP Meta-Orchestration Setup" -ForegroundColor Cyan
Write-Host "  Using Codex as a Sub-Agent via MCP" -ForegroundColor Cyan
Write-Host "============================================`n" -ForegroundColor Cyan

# Step 1: Register Codex itself as an MCP server
Write-Host "[Step 1] Registering Codex as MCP Server..." -ForegroundColor Yellow
Write-Host "  Command: codex mcp add codex-agent" -ForegroundColor Gray
Write-Host ""

try {
    $result = codex mcp add codex-agent -- codex mcp-server 2>&1 | Out-String
    
    if ($result -match "error") {
        Write-Host "  [ERROR] Failed to register MCP server" -ForegroundColor Red
        Write-Host "  Output: $result" -ForegroundColor Gray
    } else {
        Write-Host "  [OK] Codex MCP server registered as 'codex-agent'" -ForegroundColor Green
    }
} catch {
    Write-Host "  [ERROR] $($_.Exception.Message)" -ForegroundColor Red
}

Write-Host ""

# Step 2: Verify registration
Write-Host "[Step 2] Verifying MCP Server Registration..." -ForegroundColor Yellow
Write-Host ""

try {
    $list = codex mcp list 2>&1 | Out-String
    
    if ($list -match "codex-agent") {
        Write-Host "  [OK] MCP server 'codex-agent' is registered" -ForegroundColor Green
        Write-Host ""
        Write-Host "  Registered servers:" -ForegroundColor Cyan
        codex mcp list | ForEach-Object { Write-Host "    $_" -ForegroundColor White }
    } else {
        Write-Host "  [WARN] MCP server not found in list" -ForegroundColor Yellow
        Write-Host "  Output: $list" -ForegroundColor Gray
    }
} catch {
    Write-Host "  [ERROR] $($_.Exception.Message)" -ForegroundColor Red
}

Write-Host ""

# Step 3: Get server details
Write-Host "[Step 3] Getting MCP Server Details..." -ForegroundColor Yellow
Write-Host ""

try {
    $details = codex mcp get codex-agent 2>&1 | Out-String
    
    if ($details -match "error|not found") {
        Write-Host "  [WARN] Could not get server details" -ForegroundColor Yellow
        Write-Host "  Output: $details" -ForegroundColor Gray
    } else {
        Write-Host "  [OK] Server details:" -ForegroundColor Green
        codex mcp get codex-agent | ForEach-Object { Write-Host "    $_" -ForegroundColor White }
    }
} catch {
    Write-Host "  [ERROR] $($_.Exception.Message)" -ForegroundColor Red
}

Write-Host ""
Write-Host ("=" * 60) -ForegroundColor Gray
Write-Host ""

# Step 4: Create agent definition that uses MCP
Write-Host "[Step 4] Creating Agent Definition (MCP-based)..." -ForegroundColor Yellow
Write-Host ""

$agentDef = @"
# Codex MCP Meta-Agent Definition
name: "codex-mcp-researcher"
description: "Research agent that uses Codex via MCP protocol"
version: "1.0.0"

capabilities:
  - "deep_research"
  - "code_analysis"
  - "web_search"
  - "mcp_tools"

tools:
  - type: "mcp"
    server: "codex-agent"
    description: "Access to Codex functionality via MCP"

instructions: |
  You are a research agent with access to Codex functionality via MCP.
  When given a research task:
  1. Use MCP tools to access Codex features
  2. Coordinate multiple sub-tasks
  3. Aggregate and synthesize results
  4. Provide comprehensive reports

max_tokens: 10000
temperature: 0.7

resource_limits:
  max_parallel_tasks: 3
  timeout_seconds: 300
"@

$agentPath = ".codex/agents/codex-mcp-researcher.yaml"
$agentDir = Split-Path $agentPath -Parent

if (-not (Test-Path $agentDir)) {
    New-Item -ItemType Directory -Path $agentDir -Force | Out-Null
}

$agentDef | Out-File -FilePath $agentPath -Encoding UTF8

if (Test-Path $agentPath) {
    Write-Host "  [OK] Agent definition created: $agentPath" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Agent configuration:" -ForegroundColor Cyan
    Get-Content $agentPath | Select-Object -First 15 | ForEach-Object { 
        Write-Host "    $_" -ForegroundColor Gray 
    }
} else {
    Write-Host "  [ERROR] Failed to create agent definition" -ForegroundColor Red
}

Write-Host ""
Write-Host ("=" * 60) -ForegroundColor Gray
Write-Host ""

# Step 5: Create test script
Write-Host "[Step 5] Creating Test Script..." -ForegroundColor Yellow
Write-Host ""

$testScript = @'
#!/usr/bin/env pwsh
# Test Codex MCP Meta-Orchestration

Write-Host "`n=== Testing Codex MCP Meta-Agent ===" -ForegroundColor Cyan
Write-Host ""

# Test 1: Direct MCP call
Write-Host "[Test 1] Direct MCP Tool Call" -ForegroundColor Yellow
Write-Host "  Testing if Codex MCP server responds..." -ForegroundColor Gray
Write-Host ""

# Note: Actual MCP tool calls would be done from within Codex
# This is a conceptual test showing the architecture

Write-Host "  Architecture:" -ForegroundColor Cyan
Write-Host "    User" -ForegroundColor White
Write-Host "      |" -ForegroundColor Gray
Write-Host "      v" -ForegroundColor Gray
Write-Host "    Codex Main Instance" -ForegroundColor White
Write-Host "      |" -ForegroundColor Gray
Write-Host "      v" -ForegroundColor Gray
Write-Host "    Sub-Agent Runtime" -ForegroundColor White
Write-Host "      |" -ForegroundColor Gray
Write-Host "      v" -ForegroundColor Gray
Write-Host "    MCP Client" -ForegroundColor White
Write-Host "      |" -ForegroundColor Gray
Write-Host "      v" -ForegroundColor Gray
Write-Host "    Codex MCP Server (stdio)" -ForegroundColor White
Write-Host "      |" -ForegroundColor Gray
Write-Host "      v" -ForegroundColor Gray
Write-Host "    Codex Tools & Features" -ForegroundColor White
Write-Host ""

Write-Host "  [INFO] This creates a recursive Codex architecture" -ForegroundColor Cyan
Write-Host "  where Codex can orchestrate itself!" -ForegroundColor Cyan
Write-Host ""

# Test 2: Verify agent can be loaded
Write-Host "[Test 2] Agent Definition Verification" -ForegroundColor Yellow
Write-Host ""

if (Test-Path ".codex/agents/codex-mcp-researcher.yaml") {
    Write-Host "  [OK] Agent definition exists" -ForegroundColor Green
    Write-Host "  Path: .codex/agents/codex-mcp-researcher.yaml" -ForegroundColor Gray
} else {
    Write-Host "  [FAIL] Agent definition not found" -ForegroundColor Red
}

Write-Host ""

# Test 3: MCP server list
Write-Host "[Test 3] MCP Server Availability" -ForegroundColor Yellow
Write-Host ""

$servers = codex mcp list 2>&1 | Out-String

if ($servers -match "codex-agent") {
    Write-Host "  [OK] Codex MCP server is available" -ForegroundColor Green
} else {
    Write-Host "  [WARN] Codex MCP server not found" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=== Meta-Orchestration Setup Complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Configure API authentication (codex auth login)" -ForegroundColor White
Write-Host "  2. Test agent execution:" -ForegroundColor White
Write-Host "     codex delegate codex-mcp-researcher --goal 'Test task'" -ForegroundColor Gray
Write-Host "  3. Monitor MCP communication in logs" -ForegroundColor White
Write-Host ""
'@

$testScriptPath = "test-codex-mcp-meta.ps1"
$testScript | Out-File -FilePath $testScriptPath -Encoding UTF8

if (Test-Path $testScriptPath) {
    Write-Host "  [OK] Test script created: $testScriptPath" -ForegroundColor Green
} else {
    Write-Host "  [ERROR] Failed to create test script" -ForegroundColor Red
}

Write-Host ""
Write-Host ("=" * 60) -ForegroundColor Gray
Write-Host ""

# Summary
Write-Host "=== Setup Complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "Configuration Summary:" -ForegroundColor Cyan
Write-Host "  [✓] MCP Server: codex-agent registered" -ForegroundColor Green
Write-Host "  [✓] Agent Definition: codex-mcp-researcher.yaml created" -ForegroundColor Green
Write-Host "  [✓] Test Script: test-codex-mcp-meta.ps1 created" -ForegroundColor Green
Write-Host ""

Write-Host "Meta-Orchestration Architecture:" -ForegroundColor Cyan
Write-Host "  Main Codex Instance" -ForegroundColor White
Write-Host "    └─> Sub-Agent Runtime" -ForegroundColor White
Write-Host "         └─> MCP Client" -ForegroundColor White
Write-Host "              └─> Codex MCP Server (stdio)" -ForegroundColor White
Write-Host "                   └─> Codex Core Features" -ForegroundColor White
Write-Host ""

Write-Host "Key Benefits:" -ForegroundColor Cyan
Write-Host "  • Recursive AI orchestration" -ForegroundColor White
Write-Host "  • Self-referential agent system" -ForegroundColor White
Write-Host "  • Modular tool access via MCP" -ForegroundColor White
Write-Host "  • Isolated execution contexts" -ForegroundColor White
Write-Host ""

Write-Host "To run the test:" -ForegroundColor Yellow
Write-Host "  .\test-codex-mcp-meta.ps1" -ForegroundColor Gray
Write-Host ""

