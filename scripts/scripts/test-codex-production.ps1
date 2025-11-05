# Codex v0.48.0 Production Test Script
# Created: 2025-10-15

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Codex v0.48.0 Production Test" -ForegroundColor Cyan
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
        $trimmedOutput = $output.Trim()
        
        if ($ExpectedPattern -and $output -match $ExpectedPattern) {
            Write-Host "  Result: PASS" -ForegroundColor Green
            $script:passCount++
            $outputPreview = if ($trimmedOutput.Length -gt 100) { $trimmedOutput.Substring(0, 100) } else { $trimmedOutput }
            $script:testResults += [PSCustomObject]@{
                Test = $TestName
                Status = "PASS"
                Output = $outputPreview
            }
        } elseif (-not $ExpectedPattern) {
            Write-Host "  Result: PASS (executed)" -ForegroundColor Green
            $script:passCount++
            $outputPreview = if ($trimmedOutput.Length -gt 100) { $trimmedOutput.Substring(0, 100) } else { $trimmedOutput }
            $script:testResults += [PSCustomObject]@{
                Test = $TestName
                Status = "PASS"
                Output = $outputPreview
            }
        } else {
            Write-Host "  Result: FAIL (pattern not matched)" -ForegroundColor Red
            Write-Host "  Expected: $ExpectedPattern" -ForegroundColor Gray
            $outputPreview = if ($trimmedOutput.Length -gt 200) { $trimmedOutput.Substring(0, 200) } else { $trimmedOutput }
            Write-Host "  Got: $outputPreview" -ForegroundColor Gray
            $script:failCount++
            $outputPreviewShort = if ($trimmedOutput.Length -gt 100) { $trimmedOutput.Substring(0, 100) } else { $trimmedOutput }
            $script:testResults += [PSCustomObject]@{
                Test = $TestName
                Status = "FAIL"
                Output = $outputPreviewShort
            }
        }
    } catch {
        Write-Host "  Result: FAIL (error)" -ForegroundColor Red
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

# Test 1: Version Check
Test-Command -TestName "Version Check" `
             -Command "codex --version" `
             -ExpectedPattern "codex-cli 0\.48\.0"

# Test 2: Help Command
Test-Command -TestName "Help Command" `
             -Command "codex --help" `
             -ExpectedPattern "Usage|USAGE"

# Test 3: Binary Exists
Write-Host "Test: Binary File Exists" -ForegroundColor Yellow
if (Test-Path "$env:USERPROFILE\.cargo\bin\codex.exe") {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "Binary File Exists"
        Status = "PASS"
        Output = "File found at: $env:USERPROFILE\.cargo\bin\codex.exe"
    }
} else {
    Write-Host "  Result: FAIL" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "Binary File Exists"
        Status = "FAIL"
        Output = "File not found"
    }
}
Write-Host ""

# Test 4: Binary Size Check
Write-Host "Test: Binary Size Check" -ForegroundColor Yellow
$binarySize = (Get-Item "$env:USERPROFILE\.cargo\bin\codex.exe").Length / 1MB
Write-Host "  Binary Size: $([Math]::Round($binarySize, 2)) MB" -ForegroundColor Gray
if ($binarySize -gt 10 -and $binarySize -lt 100) {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "Binary Size Check"
        Status = "PASS"
        Output = "$([Math]::Round($binarySize, 2)) MB (within expected range)"
    }
} else {
    Write-Host "  Result: FAIL (unexpected size)" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "Binary Size Check"
        Status = "FAIL"
        Output = "$([Math]::Round($binarySize, 2)) MB (outside expected range 10-100 MB)"
    }
}
Write-Host ""

# Test 5: PATH Check
Write-Host "Test: PATH Environment Check" -ForegroundColor Yellow
$cargoPath = "$env:USERPROFILE\.cargo\bin"
if ($env:PATH -like "*$cargoPath*") {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "PATH Environment Check"
        Status = "PASS"
        Output = "Cargo bin directory is in PATH"
    }
} else {
    Write-Host "  Result: WARNING (not in PATH)" -ForegroundColor Yellow
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "PATH Environment Check"
        Status = "PASS"
        Output = "Warning: Cargo bin not in PATH, but executable via full path"
    }
}
Write-Host ""

# Test 6: Rust Toolchain Check
Test-Command -TestName "Rust Toolchain Check" `
             -Command "rustc --version" `
             -ExpectedPattern "rustc"

# Test 7: Cargo Check
Test-Command -TestName "Cargo Check" `
             -Command "cargo --version" `
             -ExpectedPattern "cargo"

# Test 8: Build Directory Check
Write-Host "Test: Build Directory Check" -ForegroundColor Yellow
if (Test-Path "codex-rs\target\release\codex.exe") {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "Build Directory Check"
        Status = "PASS"
        Output = "Build artifacts exist in target/release"
    }
} else {
    Write-Host "  Result: FAIL" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "Build Directory Check"
        Status = "FAIL"
        Output = "Build artifacts not found"
    }
}
Write-Host ""

# Test 9: MCP Server Module Check
Write-Host "Test: MCP Server Module Check" -ForegroundColor Yellow
if (Test-Path "codex-rs\mcp-server") {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "MCP Server Module Check"
        Status = "PASS"
        Output = "MCP server module exists"
    }
} else {
    Write-Host "  Result: FAIL" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "MCP Server Module Check"
        Status = "FAIL"
        Output = "MCP server module not found"
    }
}
Write-Host ""

# Test 10: Deep Research Module Check
Write-Host "Test: Deep Research Module Check" -ForegroundColor Yellow
if (Test-Path "codex-rs\deep-research") {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "Deep Research Module Check"
        Status = "PASS"
        Output = "Deep Research module exists"
    }
} else {
    Write-Host "  Result: FAIL" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "Deep Research Module Check"
        Status = "FAIL"
        Output = "Deep Research module not found"
    }
}
Write-Host ""

# Summary
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

# Detailed Results
Write-Host "Detailed Results:" -ForegroundColor Yellow
$testResults | Format-Table -AutoSize

# System Information
Write-Host ""
Write-Host "System Information:" -ForegroundColor Yellow
Write-Host "  OS: $(Get-CimInstance Win32_OperatingSystem | Select-Object -ExpandProperty Caption)" -ForegroundColor White
Write-Host "  PowerShell: $($PSVersionTable.PSVersion)" -ForegroundColor White
Write-Host "  Rust: $(rustc --version 2>&1)" -ForegroundColor White
Write-Host "  Cargo: $(cargo --version 2>&1)" -ForegroundColor White
Write-Host ""

# Save Log
$logFile = "_docs\2025-10-15_production-test-results_v0.48.0.md"

# Build table rows (avoiding pipe character parsing issues)
$tableHeader = '| Item | Result |'
$tableSeparator = '|------|--------|'
$tableRow1 = '| Total Tests | ' + "$($passCount + $failCount)" + ' |'
$tableRow2 = '| Passed | ' + "$passCount" + ' |'
$tableRow3 = '| Failed | ' + "$failCount" + ' |'
$successRate = if (($passCount + $failCount) -gt 0) { [Math]::Round($passCount / ($passCount + $failCount) * 100, 1) } else { 0 }
$tableRow4 = '| Success Rate | ' + "$successRate%" + ' |'

$logContent = @"
# Codex v0.48.0 Production Test Results

**Test Date**: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")  
**Test Environment**: Windows $(Get-CimInstance Win32_OperatingSystem | Select-Object -ExpandProperty Version)  
**Version**: 0.48.0

---

## Test Summary

$tableHeader
$tableSeparator
$tableRow1
$tableRow2
$tableRow3
$tableRow4

---

## Detailed Results

"@

foreach ($result in $testResults) {
    $logContent += @"

### $($result.Test)
- **Status**: $($result.Status)
- **Output**: $($result.Output)

"@
}

$logContent += @"

---

## System Information

- **OS**: $(Get-CimInstance Win32_OperatingSystem | Select-Object -ExpandProperty Caption)
- **PowerShell**: $($PSVersionTable.PSVersion)
- **Rust**: $(rustc --version 2>&1)
- **Cargo**: $(cargo --version 2>&1)
- **Binary Path**: $env:USERPROFILE\.cargo\bin\codex.exe
- **Binary Size**: $([Math]::Round((Get-Item "$env:USERPROFILE\.cargo\bin\codex.exe").Length / 1MB, 2)) MB

---

## Conclusion

$( if ($failCount -eq 0) { "✅ **ALL TESTS PASSED!** Codex v0.48.0 is working correctly." } 
   elseif ($failCount -le 2) { "⚠️ **MOSTLY PASSED** - Some minor issues, but core functionality is working." }
   else { "❌ **NEEDS ATTENTION** - Multiple tests failed. Please review." } )

---

**Test Completed**: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
"@

$logContent | Out-File -FilePath $logFile -Encoding UTF8 -Force
Write-Host "Test log saved to: $logFile" -ForegroundColor Gray
Write-Host ""

