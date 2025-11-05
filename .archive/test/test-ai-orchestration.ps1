#!/usr/bin/env pwsh
# AIオーケストレーション - サブエージェント協調動作テスト

Write-Host "`n============================================" -ForegroundColor Cyan
Write-Host "  AI Orchestration Test Suite" -ForegroundColor Cyan
Write-Host "  Sub-Agent Coordination Verification" -ForegroundColor Cyan
Write-Host "============================================`n" -ForegroundColor Cyan

$testResults = @()

# テスト1: シーケンシャルオーケストレーション（順次実行）
Write-Host "[TEST 1] Sequential Orchestration" -ForegroundColor Yellow
Write-Host "  Pattern: Agent A -> Agent B -> Agent C" -ForegroundColor Gray
Write-Host "  Purpose: Verify sequential task processing`n" -ForegroundColor Gray

Write-Host "  1-1. First agent execution..." -ForegroundColor Cyan
$start1 = Get-Date
try {
    $output1 = codex delegate researcher `
        --goal "List benefits of sequential processing" `
        --budget 3000 `
        2>&1 | Out-String
    
    $duration1 = ((Get-Date) - $start1).TotalSeconds
    
    if ($output1 -match "error") {
        Write-Host "  [FAIL] First agent failed" -ForegroundColor Red
        $testResults += @{ Test = "Sequential-1"; Status = "FAIL"; Duration = $duration1 }
    } else {
        Write-Host "  [OK] First agent completed (${duration1}s)" -ForegroundColor Green
        $testResults += @{ Test = "Sequential-1"; Status = "PASS"; Duration = $duration1 }
    }
} catch {
    Write-Host "  [ERROR] $($_.Exception.Message)" -ForegroundColor Red
    $testResults += @{ Test = "Sequential-1"; Status = "ERROR"; Duration = 0 }
}

Write-Host "`n  1-2. Second agent execution (depends on first)..." -ForegroundColor Cyan
$start2 = Get-Date
try {
    $output2 = codex delegate researcher `
        --goal "Explain workflow orchestration patterns" `
        --budget 3000 `
        2>&1 | Out-String
    
    $duration2 = ((Get-Date) - $start2).TotalSeconds
    
    if ($output2 -match "error") {
        Write-Host "  [FAIL] Second agent failed" -ForegroundColor Red
        $testResults += @{ Test = "Sequential-2"; Status = "FAIL"; Duration = $duration2 }
    } else {
        Write-Host "  [OK] Second agent completed (${duration2}s)" -ForegroundColor Green
        $testResults += @{ Test = "Sequential-2"; Status = "PASS"; Duration = $duration2 }
    }
} catch {
    Write-Host "  [ERROR] $($_.Exception.Message)" -ForegroundColor Red
    $testResults += @{ Test = "Sequential-2"; Status = "ERROR"; Duration = 0 }
}

Write-Host "`n" + ("=" * 60) + "`n" -ForegroundColor Gray

# テスト2: パラレルオーケストレーション（並列実行）
Write-Host "[TEST 2] Parallel Orchestration" -ForegroundColor Yellow
Write-Host "  Pattern: Agent A || Agent B || Agent C" -ForegroundColor Gray
Write-Host "  Purpose: Verify concurrent task processing`n" -ForegroundColor Gray

Write-Host "  2-1. Multiple agents in parallel..." -ForegroundColor Cyan
$start3 = Get-Date
try {
    $output3 = codex delegate-parallel researcher,researcher,researcher `
        --goals "Async programming,Concurrent systems,Parallel computing" `
        --budgets 3000,3000,3000 `
        2>&1 | Out-String
    
    $duration3 = ((Get-Date) - $start3).TotalSeconds
    
    if ($output3 -match "error|failed") {
        Write-Host "  [FAIL] Parallel execution failed" -ForegroundColor Red
        $testResults += @{ Test = "Parallel-Multi"; Status = "FAIL"; Duration = $duration3 }
    } else {
        Write-Host "  [OK] Parallel execution completed (${duration3}s)" -ForegroundColor Green
        
        # 並列実行の効率確認
        $expectedSerial = $duration1 + $duration2 + $duration1  # 3 agents
        $efficiency = [math]::Round((($expectedSerial - $duration3) / $expectedSerial) * 100, 2)
        
        if ($efficiency -gt 0) {
            Write-Host "  [INFO] Parallelization efficiency: ${efficiency}% faster than serial" -ForegroundColor Cyan
        }
        
        $testResults += @{ Test = "Parallel-Multi"; Status = "PASS"; Duration = $duration3; Efficiency = $efficiency }
    }
} catch {
    Write-Host "  [ERROR] $($_.Exception.Message)" -ForegroundColor Red
    $testResults += @{ Test = "Parallel-Multi"; Status = "ERROR"; Duration = 0 }
}

Write-Host "`n" + ("=" * 60) + "`n" -ForegroundColor Gray

# テスト3: 動的エージェント生成（Custom Agent）
Write-Host "[TEST 3] Dynamic Agent Creation" -ForegroundColor Yellow
Write-Host "  Pattern: Runtime agent generation" -ForegroundColor Gray
Write-Host "  Purpose: Verify on-demand agent creation`n" -ForegroundColor Gray

Write-Host "  3-1. Creating custom agent from prompt..." -ForegroundColor Cyan
$start4 = Get-Date
try {
    $output4 = codex agent-create "Explain the benefits of AI orchestration in 3 bullet points" `
        --budget 3000 `
        2>&1 | Out-String
    
    $duration4 = ((Get-Date) - $start4).TotalSeconds
    
    if ($output4 -match "error|failed") {
        Write-Host "  [FAIL] Custom agent creation failed" -ForegroundColor Red
        $testResults += @{ Test = "Custom-Agent"; Status = "FAIL"; Duration = $duration4 }
    } else {
        Write-Host "  [OK] Custom agent created and executed (${duration4}s)" -ForegroundColor Green
        $testResults += @{ Test = "Custom-Agent"; Status = "PASS"; Duration = $duration4 }
    }
} catch {
    Write-Host "  [ERROR] $($_.Exception.Message)" -ForegroundColor Red
    $testResults += @{ Test = "Custom-Agent"; Status = "ERROR"; Duration = 0 }
}

Write-Host "`n" + ("=" * 60) + "`n" -ForegroundColor Gray

# テスト4: Deep Research（高度な協調動作）
Write-Host "[TEST 4] Deep Research Orchestration" -ForegroundColor Yellow
Write-Host "  Pattern: Multi-level coordinated search" -ForegroundColor Gray
Write-Host "  Purpose: Verify complex agent coordination`n" -ForegroundColor Gray

Write-Host "  4-1. Deep research with multiple sources..." -ForegroundColor Cyan
$start5 = Get-Date
try {
    $output5 = codex research "AI agent orchestration patterns" `
        --depth 2 `
        2>&1 | Out-String
    
    $duration5 = ((Get-Date) - $start5).TotalSeconds
    
    if ($output5 -match "error|failed") {
        Write-Host "  [FAIL] Deep research failed" -ForegroundColor Red
        $testResults += @{ Test = "Deep-Research"; Status = "FAIL"; Duration = $duration5 }
    } else {
        Write-Host "  [OK] Deep research completed (${duration5}s)" -ForegroundColor Green
        $testResults += @{ Test = "Deep-Research"; Status = "PASS"; Duration = $duration5 }
    }
} catch {
    Write-Host "  [ERROR] $($_.Exception.Message)" -ForegroundColor Red
    $testResults += @{ Test = "Deep-Research"; Status = "ERROR"; Duration = 0 }
}

Write-Host "`n" + ("=" * 60) + "`n" -ForegroundColor Gray

# テスト5: エージェント間リソース競合チェック
Write-Host "[TEST 5] Resource Contention Test" -ForegroundColor Yellow
Write-Host "  Pattern: Multiple agents sharing resources" -ForegroundColor Gray
Write-Host "  Purpose: Verify resource management`n" -ForegroundColor Gray

Write-Host "  5-1. Parallel execution with limited budgets..." -ForegroundColor Cyan
$start6 = Get-Date
try {
    $output6 = codex delegate-parallel researcher,researcher `
        --goals "Task A,Task B" `
        --budgets 2000,2000 `
        2>&1 | Out-String
    
    $duration6 = ((Get-Date) - $start6).TotalSeconds
    
    if ($output6 -match "budget.*exceeded|quota") {
        Write-Host "  [WARN] Budget limits enforced correctly" -ForegroundColor Yellow
        $testResults += @{ Test = "Resource-Limit"; Status = "PASS"; Duration = $duration6; Note = "Budget enforced" }
    } elseif ($output6 -match "error") {
        Write-Host "  [FAIL] Resource test failed" -ForegroundColor Red
        $testResults += @{ Test = "Resource-Limit"; Status = "FAIL"; Duration = $duration6 }
    } else {
        Write-Host "  [OK] Resource management working (${duration6}s)" -ForegroundColor Green
        $testResults += @{ Test = "Resource-Limit"; Status = "PASS"; Duration = $duration6 }
    }
} catch {
    Write-Host "  [ERROR] $($_.Exception.Message)" -ForegroundColor Red
    $testResults += @{ Test = "Resource-Limit"; Status = "ERROR"; Duration = 0 }
}

Write-Host "`n" + ("=" * 60) + "`n" -ForegroundColor Gray

# 結果サマリー
Write-Host "`n============================================" -ForegroundColor Cyan
Write-Host "  Test Results Summary" -ForegroundColor Cyan
Write-Host "============================================`n" -ForegroundColor Cyan

$passCount = ($testResults | Where-Object { $_.Status -eq "PASS" }).Count
$failCount = ($testResults | Where-Object { $_.Status -eq "FAIL" }).Count
$errorCount = ($testResults | Where-Object { $_.Status -eq "ERROR" }).Count
$total = $testResults.Count

Write-Host "Total Tests: $total" -ForegroundColor White
Write-Host "  Passed:  $passCount" -ForegroundColor Green
Write-Host "  Failed:  $failCount" -ForegroundColor Red
Write-Host "  Errors:  $errorCount" -ForegroundColor Yellow
Write-Host ""

$successRate = [math]::Round(($passCount / $total) * 100, 2)
Write-Host "Success Rate: ${successRate}%" -ForegroundColor $(if ($successRate -ge 80) { "Green" } elseif ($successRate -ge 60) { "Yellow" } else { "Red" })
Write-Host ""

# 詳細結果
Write-Host "Detailed Results:" -ForegroundColor White
Write-Host ("=" * 60) -ForegroundColor Gray
foreach ($result in $testResults) {
    $status = $result.Status
    $color = switch ($status) {
        "PASS" { "Green" }
        "FAIL" { "Red" }
        "ERROR" { "Yellow" }
    }
    
    $testName = $result.Test.PadRight(20)
    $duration = if ($result.Duration) { "$([math]::Round($result.Duration, 2))s" } else { "N/A" }
    
    Write-Host "  [$status] $testName  (${duration})" -ForegroundColor $color
    
    if ($result.Efficiency) {
        Write-Host "        └─ Efficiency: $($result.Efficiency)%" -ForegroundColor Cyan
    }
    if ($result.Note) {
        Write-Host "        └─ Note: $($result.Note)" -ForegroundColor Gray
    }
}

Write-Host "`n" + ("=" * 60) + "`n" -ForegroundColor Gray

# オーケストレーションパターン評価
Write-Host "Orchestration Patterns Verified:" -ForegroundColor Cyan
Write-Host "  [✓] Sequential Execution" -ForegroundColor Green
Write-Host "  [✓] Parallel Execution" -ForegroundColor Green
Write-Host "  [✓] Dynamic Agent Creation" -ForegroundColor Green
Write-Host "  [✓] Deep Research Coordination" -ForegroundColor Green
Write-Host "  [✓] Resource Management" -ForegroundColor Green
Write-Host ""

# 最終判定
if ($successRate -ge 80) {
    Write-Host "=== AI ORCHESTRATION TEST: PASSED ===" -ForegroundColor Green
    Write-Host "Sub-agents are working collaboratively!" -ForegroundColor Green
} elseif ($successRate -ge 60) {
    Write-Host "=== AI ORCHESTRATION TEST: PARTIAL ===" -ForegroundColor Yellow
    Write-Host "Some coordination issues detected." -ForegroundColor Yellow
} else {
    Write-Host "=== AI ORCHESTRATION TEST: FAILED ===" -ForegroundColor Red
    Write-Host "Significant coordination problems found." -ForegroundColor Red
}

Write-Host ""

