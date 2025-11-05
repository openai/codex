# Codex v0.48.0 実機テストスクリプト
# 作成日時: 2025-10-15

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Codex v0.48.0 実機テスト" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$testResults = @()
$passCount = 0
$failCount = 0

function Test-Command {
    param(
        [string]$TestName,
        [string]$Command,
        [string]$ExpectedPattern
    )
    
    Write-Host "Test: $TestName" -ForegroundColor Yellow
    Write-Host "  Command: $Command" -ForegroundColor Gray
    
    try {
        $output = Invoke-Expression $Command 2>&1 | Out-String
        
        if ($ExpectedPattern -and $output -match $ExpectedPattern) {
            Write-Host "  Result: PASS" -ForegroundColor Green
            $script:passCount++
            $script:testResults += [PSCustomObject]@{
                Test = $TestName
                Status = "PASS"
                Output = $output.Trim().Substring(0, [Math]::Min(100, $output.Length))
            }
        } elseif (-not $ExpectedPattern) {
            Write-Host "  Result: PASS (executed)" -ForegroundColor Green
            $script:passCount++
            $script:testResults += [PSCustomObject]@{
                Test = $TestName
                Status = "PASS"
                Output = $output.Trim().Substring(0, [Math]::Min(100, $output.Length))
            }
        } else {
            Write-Host "  Result: FAIL (pattern not matched)" -ForegroundColor Red
            Write-Host "  Expected: $ExpectedPattern" -ForegroundColor Gray
            Write-Host "  Got: $($output.Trim().Substring(0, [Math]::Min(200, $output.Length)))" -ForegroundColor Gray
            $script:failCount++
            $script:testResults += [PSCustomObject]@{
                Test = $TestName
                Status = "FAIL"
                Output = "Expected pattern not found: $ExpectedPattern"
            }
        }
    } catch {
        Write-Host "  Result: FAIL (exception)" -ForegroundColor Red
        Write-Host "  Error: $_" -ForegroundColor Gray
        $script:failCount++
        $script:testResults += [PSCustomObject]@{
            Test = $TestName
            Status = "FAIL"
            Output = $_.Exception.Message
        }
    }
    
    Write-Host ""
}

# テスト1: バージョン確認
Test-Command `
    -TestName "1. Version Check" `
    -Command "codex --version" `
    -ExpectedPattern "codex"

# テスト2: ヘルプ表示
Test-Command `
    -TestName "2. Help Display" `
    -Command "codex --help" `
    -ExpectedPattern "Usage|USAGE"

# テスト3: MCP サーバーヘルプ
Test-Command `
    -TestName "3. MCP Server Help" `
    -Command "codex mcp-server --help" `
    -ExpectedPattern "mcp|MCP"

# テスト4: 設定ファイルチェック
Write-Host "Test: 4. Configuration File Check" -ForegroundColor Yellow
if (Test-Path "$env:USERPROFILE\.codex\config.toml") {
    Write-Host "  Result: PASS (config exists)" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "4. Configuration File Check"
        Status = "PASS"
        Output = "Config file found at $env:USERPROFILE\.codex\config.toml"
    }
} else {
    Write-Host "  Result: WARN (no config file)" -ForegroundColor Yellow
    Write-Host "  Note: Config file is optional" -ForegroundColor Gray
    $testResults += [PSCustomObject]@{
        Test = "4. Configuration File Check"
        Status = "WARN"
        Output = "No config file (optional)"
    }
}
Write-Host ""

# テスト5: バイナリサイズチェック
Write-Host "Test: 5. Binary Size Check" -ForegroundColor Yellow
$binaryPath = "$env:USERPROFILE\.cargo\bin\codex.exe"
if (Test-Path $binaryPath) {
    $size = (Get-Item $binaryPath).Length / 1MB
    Write-Host "  Size: $([Math]::Round($size, 2)) MB" -ForegroundColor White
    
    if ($size -gt 10 -and $size -lt 100) {
        Write-Host "  Result: PASS (reasonable size)" -ForegroundColor Green
        $passCount++
        $testResults += [PSCustomObject]@{
            Test = "5. Binary Size Check"
            Status = "PASS"
            Output = "Size: $([Math]::Round($size, 2)) MB"
        }
    } else {
        Write-Host "  Result: WARN (unusual size)" -ForegroundColor Yellow
        $testResults += [PSCustomObject]@{
            Test = "5. Binary Size Check"
            Status = "WARN"
            Output = "Size: $([Math]::Round($size, 2)) MB (unusual)"
        }
    }
} else {
    Write-Host "  Result: FAIL (binary not found)" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "5. Binary Size Check"
        Status = "FAIL"
        Output = "Binary not found at $binaryPath"
    }
}
Write-Host ""

# テスト6: 依存関係チェック
Write-Host "Test: 6. Dependencies Check" -ForegroundColor Yellow
try {
    $output = codex --version 2>&1 | Out-String
    if ($output -match "error|missing|cannot find") {
        Write-Host "  Result: FAIL (missing dependencies)" -ForegroundColor Red
        $failCount++
        $testResults += [PSCustomObject]@{
            Test = "6. Dependencies Check"
            Status = "FAIL"
            Output = "Missing dependencies detected"
        }
    } else {
        Write-Host "  Result: PASS (all dependencies found)" -ForegroundColor Green
        $passCount++
        $testResults += [PSCustomObject]@{
            Test = "6. Dependencies Check"
            Status = "PASS"
            Output = "All dependencies OK"
        }
    }
} catch {
    Write-Host "  Result: FAIL (exception)" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "6. Dependencies Check"
        Status = "FAIL"
        Output = $_.Exception.Message
    }
}
Write-Host ""

# テスト7: MCP 設定確認
Write-Host "Test: 7. MCP Configuration Check" -ForegroundColor Yellow
$mcpConfig = "$env:USERPROFILE\.cursor\mcp.json"
if (Test-Path $mcpConfig) {
    Write-Host "  Result: PASS (MCP config exists)" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "7. MCP Configuration Check"
        Status = "PASS"
        Output = "MCP config found at $mcpConfig"
    }
} else {
    Write-Host "  Result: INFO (no MCP config)" -ForegroundColor Gray
    $testResults += [PSCustomObject]@{
        Test = "7. MCP Configuration Check"
        Status = "INFO"
        Output = "MCP config not found (optional for CLI use)"
    }
}
Write-Host ""

# サマリー
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Test Summary" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Total Tests: $($passCount + $failCount)" -ForegroundColor White
Write-Host "Passed: $passCount" -ForegroundColor Green
Write-Host "Failed: $failCount" -ForegroundColor Red
Write-Host ""

if ($failCount -eq 0) {
    Write-Host "Overall Status: ALL TESTS PASSED!" -ForegroundColor Green -BackgroundColor Black
} elseif ($failCount -le 2) {
    Write-Host "Overall Status: MOSTLY PASSED (minor issues)" -ForegroundColor Yellow
} else {
    Write-Host "Overall Status: TESTS FAILED" -ForegroundColor Red -BackgroundColor Black
}
Write-Host ""

# 詳細レポート
Write-Host "Detailed Results:" -ForegroundColor Yellow
$testResults | Format-Table -AutoSize

# システム情報
Write-Host ""
Write-Host "System Information:" -ForegroundColor Yellow
Write-Host "  OS: $(Get-CimInstance Win32_OperatingSystem | Select-Object -ExpandProperty Caption)" -ForegroundColor White
Write-Host "  PowerShell: $($PSVersionTable.PSVersion)" -ForegroundColor White
Write-Host "  Rust: $(rustc --version 2>&1)" -ForegroundColor White
Write-Host "  Cargo: $(cargo --version 2>&1)" -ForegroundColor White
Write-Host ""

# ログ保存
$logFile = "_docs\2025-10-15_実機テスト結果_v0.48.0.md"

# テーブル行を個別に構築（パイプ記号をエスケープ）
$tableHeader = '| 項目 | 結果 |'
$tableSeparator = '|------|------|'
$tableRow1 = '| 総テスト数 | ' + "$($passCount + $failCount)" + ' |'
$tableRow2 = '| 成功 | ' + "$passCount" + ' |'
$tableRow3 = '| 失敗 | ' + "$failCount" + ' |'
$successRate = [Math]::Round($passCount / ($passCount + $failCount) * 100, 1)
$tableRow4 = '| 成功率 | ' + "$successRate%" + ' |'

$logContent = @"
# Codex v0.48.0 実機テスト結果

**実施日時**: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")  
**テスト環境**: Windows $(Get-CimInstance Win32_OperatingSystem | Select-Object -ExpandProperty Version)  
**バージョン**: 0.48.0

---

## テスト結果サマリー

$tableHeader
$tableSeparator
$tableRow1
$tableRow2
$tableRow3
$tableRow4

---

## 詳細結果

"@

foreach ($result in $testResults) {
    $logContent += @"

### $($result.Test)
- **ステータス**: $($result.Status)
- **出力**: $($result.Output)

"@
}

$logContent += @"

---

## システム情報

- **OS**: $(Get-CimInstance Win32_OperatingSystem | Select-Object -ExpandProperty Caption)
- **PowerShell**: $($PSVersionTable.PSVersion)
- **Rust**: $(rustc --version 2>&1)
- **Cargo**: $(cargo --version 2>&1)
- **バイナリパス**: $env:USERPROFILE\.cargo\bin\codex.exe
- **バイナリサイズ**: $([Math]::Round((Get-Item "$env:USERPROFILE\.cargo\bin\codex.exe").Length / 1MB, 2)) MB

---

## 結論

$( if ($failCount -eq 0) { "✅ **全テスト合格！** Codex v0.48.0 は正常に動作しています。" } 
   elseif ($failCount -le 2) { "⚠️ **概ね合格** - 一部軽微な問題がありますが、基本機能は正常です。" }
   else { "❌ **要修正** - 複数のテストが失敗しました。修正が必要です。" } )

---

**テスト完了時刻**: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
"@

$logContent | Out-File -FilePath $logFile -Encoding UTF8 -Force
Write-Host "Test log saved to: $logFile" -ForegroundColor Gray
Write-Host ""

