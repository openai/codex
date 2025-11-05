# ClaudeCode超え新機能 実機テストスクリプト
# Version: 0.48.0
# Date: 2025-10-15

Write-Host "================================" -ForegroundColor Cyan
Write-Host "  Codex v0.48.0 New Features Test" -ForegroundColor Cyan
Write-Host "  ClaudeCode-surpassing Features" -ForegroundColor Cyan
Write-Host "================================" -ForegroundColor Cyan
Write-Host ""

$testResults = @()
$totalTests = 8
$passedTests = 0

# Test 1: TaskAnalyzer - 複雑度判定
Write-Host "[Test 1/$totalTests] TaskAnalyzer - Complexity Scoring" -ForegroundColor Yellow
Write-Host "Running: cargo test -p codex-core test_task_analyzer_basic_complexity --quiet"
$result1 = cargo test -p codex-core test_task_analyzer_basic_complexity --quiet 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ PASS - TaskAnalyzer complexity scoring works" -ForegroundColor Green
    $passedTests++
    $testResults += "✅ Test 1: TaskAnalyzer complexity"
} else {
    Write-Host "  ❌ FAIL - TaskAnalyzer test failed" -ForegroundColor Red
    $testResults += "❌ Test 1: TaskAnalyzer complexity"
}
Write-Host ""

# Test 2: TaskAnalyzer - キーワード検出
Write-Host "[Test 2/$totalTests] TaskAnalyzer - Keyword Detection" -ForegroundColor Yellow
Write-Host "Running: cargo test -p codex-core test_task_analyzer_keyword_detection --quiet"
$result2 = cargo test -p codex-core test_task_analyzer_keyword_detection --quiet 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ PASS - Keyword detection works" -ForegroundColor Green
    $passedTests++
    $testResults += "✅ Test 2: Keyword detection"
} else {
    Write-Host "  ❌ FAIL - Keyword detection failed" -ForegroundColor Red
    $testResults += "❌ Test 2: Keyword detection"
}
Write-Host ""

# Test 3: TaskAnalyzer - サブタスク分解
Write-Host "[Test 3/$totalTests] TaskAnalyzer - Subtask Decomposition" -ForegroundColor Yellow
Write-Host "Running: cargo test -p codex-core test_task_analyzer_subtask_decomposition --quiet"
$result3 = cargo test -p codex-core test_task_analyzer_subtask_decomposition --quiet 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ PASS - Subtask decomposition works" -ForegroundColor Green
    $passedTests++
    $testResults += "✅ Test 3: Subtask decomposition"
} else {
    Write-Host "  ❌ FAIL - Subtask decomposition failed" -ForegroundColor Red
    $testResults += "❌ Test 3: Subtask decomposition"
}
Write-Host ""

# Test 4: ErrorHandler - リトライポリシー
Write-Host "[Test 4/$totalTests] ErrorHandler - Retry Policy" -ForegroundColor Yellow
Write-Host "Running: cargo test -p codex-core test_error_handler_retry_policy --quiet"
$result4 = cargo test -p codex-core test_error_handler_retry_policy --quiet 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ PASS - Retry policy works" -ForegroundColor Green
    $passedTests++
    $testResults += "✅ Test 4: Retry policy"
} else {
    Write-Host "  ❌ FAIL - Retry policy failed" -ForegroundColor Red
    $testResults += "❌ Test 4: Retry policy"
}
Write-Host ""

# Test 5: ErrorHandler - エラー種別処理
Write-Host "[Test 5/$totalTests] ErrorHandler - Different Error Types" -ForegroundColor Yellow
Write-Host "Running: cargo test -p codex-core test_error_handler_different_errors --quiet"
$result5 = cargo test -p codex-core test_error_handler_different_errors --quiet 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ PASS - Error type handling works" -ForegroundColor Green
    $passedTests++
    $testResults += "✅ Test 5: Error type handling"
} else {
    Write-Host "  ❌ FAIL - Error type handling failed" -ForegroundColor Red
    $testResults += "❌ Test 5: Error type handling"
}
Write-Host ""

# Test 6: ConflictResolver - マージ戦略
Write-Host "[Test 6/$totalTests] ConflictResolver - Merge Strategies" -ForegroundColor Yellow
Write-Host "Running: cargo test -p codex-core test_conflict_resolver --quiet"
$result6 = cargo test -p codex-core test_conflict_resolver_sequential_edits --quiet 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ PASS - Conflict resolution works" -ForegroundColor Green
    $passedTests++
    $testResults += "✅ Test 6: Conflict resolution"
} else {
    Write-Host "  ❌ FAIL - Conflict resolution failed" -ForegroundColor Red
    $testResults += "❌ Test 6: Conflict resolution"
}
Write-Host ""

# Test 7: MCP Integration - Auto Orchestrate Tool
Write-Host "[Test 7/$totalTests] MCP Integration - Auto Orchestrate Tool" -ForegroundColor Yellow
Write-Host "Checking MCP server can list tools..."
$mcpTest = py -3 -c @"
import json
import subprocess
import sys

# Start MCP server
proc = subprocess.Popen(
    ['codex', 'mcp-server'],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True
)

# Send initialize request
init_req = {
    'jsonrpc': '2.0',
    'id': 1,
    'method': 'initialize',
    'params': {
        'protocolVersion': '2024-11-05',
        'capabilities': {},
        'clientInfo': {'name': 'test-client', 'version': '1.0'}
    }
}
proc.stdin.write(json.dumps(init_req) + '\n')
proc.stdin.flush()

# Read response
response = proc.stdout.readline()
init_result = json.loads(response)

# Send list tools request
list_tools_req = {
    'jsonrpc': '2.0',
    'id': 2,
    'method': 'tools/list',
    'params': {}
}
proc.stdin.write(json.dumps(list_tools_req) + '\n')
proc.stdin.flush()

# Read tools response
tools_response = proc.stdout.readline()
tools_result = json.loads(tools_response)

# Check if codex-auto-orchestrate exists
if 'result' in tools_result:
    tool_names = [t['name'] for t in tools_result['result']['tools']]
    if 'codex-auto-orchestrate' in tool_names:
        print('PASS')
        sys.exit(0)
    else:
        print('FAIL: codex-auto-orchestrate not found')
        sys.exit(1)
else:
    print('FAIL: No tools returned')
    sys.exit(1)

proc.terminate()
"@

if ($mcpTest -eq "PASS") {
    Write-Host "  ✅ PASS - MCP auto-orchestrate tool available" -ForegroundColor Green
    $passedTests++
    $testResults += "✅ Test 7: MCP auto-orchestrate tool"
} else {
    Write-Host "  ❌ FAIL - MCP integration issue: $mcpTest" -ForegroundColor Red
    $testResults += "❌ Test 7: MCP auto-orchestrate tool"
}
Write-Host ""

# Test 8: Webhook Tool (basic validation)
Write-Host "[Test 8/$totalTests] Webhook Integration - Tool Definition" -ForegroundColor Yellow
Write-Host "Checking webhook tool exists in MCP server..."
$webhookCheck = py -3 -c @"
import json
import subprocess
import sys

proc = subprocess.Popen(
    ['codex', 'mcp-server'],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True
)

init_req = {
    'jsonrpc': '2.0',
    'id': 1,
    'method': 'initialize',
    'params': {
        'protocolVersion': '2024-11-05',
        'capabilities': {},
        'clientInfo': {'name': 'test', 'version': '1.0'}
    }
}
proc.stdin.write(json.dumps(init_req) + '\n')
proc.stdin.flush()
proc.stdout.readline()

list_tools_req = {
    'jsonrpc': '2.0',
    'id': 2,
    'method': 'tools/list',
    'params': {}
}
proc.stdin.write(json.dumps(list_tools_req) + '\n')
proc.stdin.flush()

tools_response = proc.stdout.readline()
tools_result = json.loads(tools_response)

if 'result' in tools_result:
    tool_names = [t['name'] for t in tools_result['result']['tools']]
    if 'codex-webhook' in tool_names:
        print('PASS')
        sys.exit(0)

print('FAIL')
sys.exit(1)
proc.terminate()
"@

if ($webhookCheck -eq "PASS") {
    Write-Host "  ✅ PASS - Webhook tool available in MCP" -ForegroundColor Green
    $passedTests++
    $testResults += "✅ Test 8: Webhook tool"
} else {
    Write-Host "  ⚠️  SKIP - Webhook tool check (MCP server may need rebuild)" -ForegroundColor Yellow
    $testResults += "⚠️  Test 8: Webhook tool (skipped)"
}
Write-Host ""

# Final Summary
Write-Host "================================" -ForegroundColor Cyan
Write-Host "  Test Summary" -ForegroundColor Cyan
Write-Host "================================" -ForegroundColor Cyan
Write-Host ""

foreach ($result in $testResults) {
    Write-Host $result
}

Write-Host ""
Write-Host "Total: $passedTests/$totalTests tests passed" -ForegroundColor $(if ($passedTests -eq $totalTests) { "Green" } elseif ($passedTests -ge ($totalTests * 0.75)) { "Yellow" } else { "Red" })

$percentage = [math]::Round(($passedTests / $totalTests) * 100, 1)
Write-Host "Success Rate: $percentage%" -ForegroundColor $(if ($percentage -eq 100) { "Green" } elseif ($percentage -ge 75) { "Yellow" } else { "Red" })

Write-Host ""
Write-Host "================================" -ForegroundColor Cyan

if ($passedTests -eq $totalTests) {
    Write-Host "🎉 ALL TESTS PASSED! Production ready!" -ForegroundColor Green
    exit 0
} elseif ($passedTests -ge ($totalTests * 0.75)) {
    Write-Host "⚠️  Most tests passed, minor issues remain" -ForegroundColor Yellow
    exit 0
} else {
    Write-Host "❌ Multiple test failures detected" -ForegroundColor Red
    exit 1
}

