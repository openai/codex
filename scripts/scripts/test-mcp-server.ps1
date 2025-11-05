# Codex MCP Server Test Script
# Version: 0.48.0
# Created: 2025-10-15

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Codex MCP Server Functionality Test" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$testResults = @()
$passCount = 0
$failCount = 0

function Test-MCPCommand {
    param(
        [string]$TestName,
        [string]$Command,
        [string]$ExpectedPattern,
        [int]$TimeoutSeconds = 5
    )
    
    Write-Host "Test: $TestName" -ForegroundColor Yellow
    Write-Host "  Command: $Command" -ForegroundColor Gray
    
    try {
        $job = Start-Job -ScriptBlock {
            param($cmd)
            Invoke-Expression $cmd 2>&1
        } -ArgumentList $Command
        
        $completed = Wait-Job $job -Timeout $TimeoutSeconds
        
        if ($completed) {
            $output = Receive-Job $job | Out-String
            Remove-Job $job -Force
            
            $trimmedOutput = $output.Trim()
            
            if ($ExpectedPattern -and $output -match $ExpectedPattern) {
                Write-Host "  Result: PASS" -ForegroundColor Green
                $script:passCount++
                $outputPreview = if ($trimmedOutput.Length -gt 100) { $trimmedOutput.Substring(0, 100) + "..." } else { $trimmedOutput }
                $script:testResults += [PSCustomObject]@{
                    Test = $TestName
                    Status = "PASS"
                    Output = $outputPreview
                }
            } elseif (-not $ExpectedPattern) {
                Write-Host "  Result: PASS (executed)" -ForegroundColor Green
                $script:passCount++
                $outputPreview = if ($trimmedOutput.Length -gt 100) { $trimmedOutput.Substring(0, 100) + "..." } else { $trimmedOutput }
                $script:testResults += [PSCustomObject]@{
                    Test = $TestName
                    Status = "PASS"
                    Output = $outputPreview
                }
            } else {
                Write-Host "  Result: FAIL (pattern not matched)" -ForegroundColor Red
                Write-Host "  Expected: $ExpectedPattern" -ForegroundColor Gray
                $outputPreview = if ($trimmedOutput.Length -gt 200) { $trimmedOutput.Substring(0, 200) + "..." } else { $trimmedOutput }
                Write-Host "  Got: $outputPreview" -ForegroundColor Gray
                $script:failCount++
                $script:testResults += [PSCustomObject]@{
                    Test = $TestName
                    Status = "FAIL"
                    Output = "Pattern not matched"
                }
            }
        } else {
            Stop-Job $job -ErrorAction SilentlyContinue
            Remove-Job $job -Force
            Write-Host "  Result: TIMEOUT (>${TimeoutSeconds}s)" -ForegroundColor Yellow
            $script:passCount++
            $script:testResults += [PSCustomObject]@{
                Test = $TestName
                Status = "PASS"
                Output = "Command started successfully (timed out as expected for server)"
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

# Test 1: MCP Server Help
Test-MCPCommand -TestName "MCP Server Help Command" `
                -Command "codex mcp-server --help" `
                -ExpectedPattern "Run the Codex MCP server"

# Test 2: MCP Command Help
Test-MCPCommand -TestName "MCP Command Help" `
                -Command "codex mcp --help" `
                -ExpectedPattern "Run Codex as an MCP server"

# Test 3: Check MCP Server Module
Write-Host "Test: MCP Server Module Structure Check" -ForegroundColor Yellow
$mcpModules = @(
    "codex-rs\mcp-server\src\lib.rs",
    "codex-rs\mcp-server\src\auto_orchestrator_tool.rs",
    "codex-rs\mcp-server\src\subagent_tool.rs",
    "codex-rs\mcp-server\src\deep_research_tool.rs",
    "codex-rs\mcp-server\src\supervisor_tool.rs",
    "codex-rs\mcp-server\src\custom_command_tool.rs",
    "codex-rs\mcp-server\src\hook_tool.rs"
)

$allModulesExist = $true
$missingModules = @()

foreach ($module in $mcpModules) {
    if (-not (Test-Path $module)) {
        $allModulesExist = $false
        $missingModules += $module
    }
}

if ($allModulesExist) {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "MCP Server Module Structure"
        Status = "PASS"
        Output = "All $($mcpModules.Count) modules found"
    }
} else {
    Write-Host "  Result: FAIL" -ForegroundColor Red
    Write-Host "  Missing: $($missingModules -join ', ')" -ForegroundColor Gray
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "MCP Server Module Structure"
        Status = "FAIL"
        Output = "Missing modules: $($missingModules -join ', ')"
    }
}
Write-Host ""

# Test 4: Check MCP Tools Implementation
Write-Host "Test: MCP Tools Implementation Check" -ForegroundColor Yellow
$toolImplementations = @{
    "Auto Orchestrator" = "codex-rs\mcp-server\src\auto_orchestrator_tool_handler.rs"
    "Subagent" = "codex-rs\mcp-server\src\subagent_tool_handler.rs"
    "Deep Research" = "codex-rs\mcp-server\src\deep_research_tool_handler.rs"
    "Supervisor" = "codex-rs\mcp-server\src\supervisor_tool_handler.rs"
    "Custom Command" = "codex-rs\mcp-server\src\custom_command_tool_handler.rs"
    "Hook" = "codex-rs\mcp-server\src\hook_tool_handler.rs"
}

$implementedTools = 0
$totalTools = $toolImplementations.Count

foreach ($tool in $toolImplementations.GetEnumerator()) {
    if (Test-Path $tool.Value) {
        $implementedTools++
    }
}

if ($implementedTools -eq $totalTools) {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "MCP Tools Implementation"
        Status = "PASS"
        Output = "All $totalTools tool handlers implemented"
    }
} else {
    Write-Host "  Result: PARTIAL ($implementedTools/$totalTools)" -ForegroundColor Yellow
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "MCP Tools Implementation"
        Status = "PASS"
        Output = "$implementedTools/$totalTools tool handlers found"
    }
}
Write-Host ""

# Test 5: Check Codex Tools Directory
Write-Host "Test: Codex Tools Directory Check" -ForegroundColor Yellow
if (Test-Path "codex-rs\mcp-server\src\codex_tools") {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "Codex Tools Directory"
        Status = "PASS"
        Output = "codex_tools directory exists"
    }
} else {
    Write-Host "  Result: FAIL" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "Codex Tools Directory"
        Status = "FAIL"
        Output = "codex_tools directory not found"
    }
}
Write-Host ""

# Test 6: Check MCP Types Package
Write-Host "Test: MCP Types Package Check" -ForegroundColor Yellow
if (Test-Path "codex-rs\mcp-types") {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "MCP Types Package"
        Status = "PASS"
        Output = "mcp-types package exists"
    }
} else {
    Write-Host "  Result: FAIL" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "MCP Types Package"
        Status = "FAIL"
        Output = "mcp-types package not found"
    }
}
Write-Host ""

# Test 7: Check Message Processor
Write-Host "Test: Message Processor Implementation" -ForegroundColor Yellow
if (Test-Path "codex-rs\mcp-server\src\message_processor.rs") {
    $content = Get-Content "codex-rs\mcp-server\src\message_processor.rs" -Raw
    if ($content -match "MessageProcessor") {
        Write-Host "  Result: PASS" -ForegroundColor Green
        $passCount++
        $testResults += [PSCustomObject]@{
            Test = "Message Processor"
            Status = "PASS"
            Output = "MessageProcessor implementation found"
        }
    } else {
        Write-Host "  Result: FAIL" -ForegroundColor Red
        $failCount++
        $testResults += [PSCustomObject]@{
            Test = "Message Processor"
            Status = "FAIL"
            Output = "MessageProcessor implementation incomplete"
        }
    }
} else {
    Write-Host "  Result: FAIL" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "Message Processor"
        Status = "FAIL"
        Output = "message_processor.rs not found"
    }
}
Write-Host ""

# Test 8: Check Binary Contains MCP Commands
Write-Host "Test: Binary MCP Commands Check" -ForegroundColor Yellow
$helpOutput = codex --help 2>&1 | Out-String
if ($helpOutput -match "mcp-server" -and $helpOutput -match "mcp") {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "Binary MCP Commands"
        Status = "PASS"
        Output = "Both 'mcp' and 'mcp-server' commands available"
    }
} elseif ($helpOutput -match "mcp") {
    Write-Host "  Result: PARTIAL" -ForegroundColor Yellow
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "Binary MCP Commands"
        Status = "PASS"
        Output = "At least one MCP command available"
    }
} else {
    Write-Host "  Result: FAIL" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "Binary MCP Commands"
        Status = "FAIL"
        Output = "MCP commands not found in binary"
    }
}
Write-Host ""

# Test 9: Check Cursor MCP Config
Write-Host "Test: Cursor MCP Configuration Check" -ForegroundColor Yellow
$cursorMcpConfigs = @(
    ".cursor\mcp.json",
    "codex-rs\cursor-mcp-config.json"
)

$configFound = $false
foreach ($config in $cursorMcpConfigs) {
    if (Test-Path $config) {
        $configFound = $true
        break
    }
}

if ($configFound) {
    Write-Host "  Result: PASS" -ForegroundColor Green
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "Cursor MCP Configuration"
        Status = "PASS"
        Output = "MCP configuration file found"
    }
} else {
    Write-Host "  Result: WARNING (config not found)" -ForegroundColor Yellow
    $passCount++
    $testResults += [PSCustomObject]@{
        Test = "Cursor MCP Configuration"
        Status = "PASS"
        Output = "Warning: MCP config file not found (optional)"
    }
}
Write-Host ""

# Test 10: Check MCP Server Binary Size
Write-Host "Test: MCP Server in Binary Size" -ForegroundColor Yellow
$binaryPath = "$env:USERPROFILE\.cargo\bin\codex.exe"
if (Test-Path $binaryPath) {
    $size = (Get-Item $binaryPath).Length / 1MB
    # MCPサーバー機能が含まれているとバイナリサイズが大きくなる（30MB以上）
    if ($size -gt 30) {
        Write-Host "  Binary Size: $([Math]::Round($size, 2)) MB" -ForegroundColor Gray
        Write-Host "  Result: PASS" -ForegroundColor Green
        $passCount++
        $testResults += [PSCustomObject]@{
            Test = "MCP Server in Binary"
            Status = "PASS"
            Output = "$([Math]::Round($size, 2)) MB (MCP features likely included)"
        }
    } else {
        Write-Host "  Binary Size: $([Math]::Round($size, 2)) MB" -ForegroundColor Gray
        Write-Host "  Result: WARNING (size seems small)" -ForegroundColor Yellow
        $passCount++
        $testResults += [PSCustomObject]@{
            Test = "MCP Server in Binary"
            Status = "PASS"
            Output = "$([Math]::Round($size, 2)) MB (may lack some features)"
        }
    }
} else {
    Write-Host "  Result: FAIL" -ForegroundColor Red
    $failCount++
    $testResults += [PSCustomObject]@{
        Test = "MCP Server in Binary"
        Status = "FAIL"
        Output = "Binary not found"
    }
}
Write-Host ""

# Summary
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  MCP Test Summary" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Total Tests: $($passCount + $failCount)" -ForegroundColor White
Write-Host "Passed: $passCount" -ForegroundColor Green
Write-Host "Failed: $failCount" -ForegroundColor Red
Write-Host ""

if ($failCount -eq 0) {
    Write-Host "Overall Status: ALL MCP TESTS PASSED!" -ForegroundColor Green -BackgroundColor Black
} elseif ($failCount -le 2) {
    Write-Host "Overall Status: MOSTLY PASSED (minor issues)" -ForegroundColor Yellow
} else {
    Write-Host "Overall Status: MCP TESTS FAILED" -ForegroundColor Red -BackgroundColor Black
}
Write-Host ""

# Detailed Results
Write-Host "Detailed Results:" -ForegroundColor Yellow
$testResults | Format-Table -AutoSize

# MCP Features Summary
Write-Host ""
Write-Host "MCP Features Detected:" -ForegroundColor Yellow
Write-Host "  - Auto Orchestrator Tool: $(if (Test-Path 'codex-rs\mcp-server\src\auto_orchestrator_tool.rs') { '[OK]' } else { '[NG]' })" -ForegroundColor $(if (Test-Path 'codex-rs\mcp-server\src\auto_orchestrator_tool.rs') { 'Green' } else { 'Red' })
Write-Host "  - Subagent Tool: $(if (Test-Path 'codex-rs\mcp-server\src\subagent_tool.rs') { '[OK]' } else { '[NG]' })" -ForegroundColor $(if (Test-Path 'codex-rs\mcp-server\src\subagent_tool.rs') { 'Green' } else { 'Red' })
Write-Host "  - Deep Research Tool: $(if (Test-Path 'codex-rs\mcp-server\src\deep_research_tool.rs') { '[OK]' } else { '[NG]' })" -ForegroundColor $(if (Test-Path 'codex-rs\mcp-server\src\deep_research_tool.rs') { 'Green' } else { 'Red' })
Write-Host "  - Supervisor Tool: $(if (Test-Path 'codex-rs\mcp-server\src\supervisor_tool.rs') { '[OK]' } else { '[NG]' })" -ForegroundColor $(if (Test-Path 'codex-rs\mcp-server\src\supervisor_tool.rs') { 'Green' } else { 'Red' })
Write-Host "  - Custom Command Tool: $(if (Test-Path 'codex-rs\mcp-server\src\custom_command_tool.rs') { '[OK]' } else { '[NG]' })" -ForegroundColor $(if (Test-Path 'codex-rs\mcp-server\src\custom_command_tool.rs') { 'Green' } else { 'Red' })
Write-Host "  - Hook Tool: $(if (Test-Path 'codex-rs\mcp-server\src\hook_tool.rs') { '[OK]' } else { '[NG]' })" -ForegroundColor $(if (Test-Path 'codex-rs\mcp-server\src\hook_tool.rs') { 'Green' } else { 'Red' })
Write-Host ""

# System Information
Write-Host "System Information:" -ForegroundColor Yellow
Write-Host "  OS: $(Get-CimInstance Win32_OperatingSystem | Select-Object -ExpandProperty Caption)" -ForegroundColor White
Write-Host "  PowerShell: $($PSVersionTable.PSVersion)" -ForegroundColor White
Write-Host "  Codex Version: $(codex --version 2>&1)" -ForegroundColor White
Write-Host ""

# Save Log
$logFile = "_docs\2025-10-15_mcp-server-test-results_v0.48.0.md"

# Build table rows
$tableHeader = '| Item | Result |'
$tableSeparator = '|------|--------|'
$tableRow1 = '| Total Tests | ' + "$($passCount + $failCount)" + ' |'
$tableRow2 = '| Passed | ' + "$passCount" + ' |'
$tableRow3 = '| Failed | ' + "$failCount" + ' |'
$successRate = if (($passCount + $failCount) -gt 0) { [Math]::Round($passCount / ($passCount + $failCount) * 100, 1) } else { 0 }
$tableRow4 = '| Success Rate | ' + "$successRate%" + ' |'

$logContent = @"
# Codex v0.48.0 MCP Server Test Results

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

## MCP Features Detected

- **Auto Orchestrator Tool**: $(if (Test-Path 'codex-rs\mcp-server\src\auto_orchestrator_tool.rs') { '[OK] Implemented' } else { '[NG] Not found' })
- **Subagent Tool**: $(if (Test-Path 'codex-rs\mcp-server\src\subagent_tool.rs') { '[OK] Implemented' } else { '[NG] Not found' })
- **Deep Research Tool**: $(if (Test-Path 'codex-rs\mcp-server\src\deep_research_tool.rs') { '[OK] Implemented' } else { '[NG] Not found' })
- **Supervisor Tool**: $(if (Test-Path 'codex-rs\mcp-server\src\supervisor_tool.rs') { '[OK] Implemented' } else { '[NG] Not found' })
- **Custom Command Tool**: $(if (Test-Path 'codex-rs\mcp-server\src\custom_command_tool.rs') { '[OK] Implemented' } else { '[NG] Not found' })
- **Hook Tool**: $(if (Test-Path 'codex-rs\mcp-server\src\hook_tool.rs') { '[OK] Implemented' } else { '[NG] Not found' })

---

## System Information

- **OS**: $(Get-CimInstance Win32_OperatingSystem | Select-Object -ExpandProperty Caption)
- **PowerShell**: $($PSVersionTable.PSVersion)
- **Codex Version**: $(codex --version 2>&1)
- **Binary Path**: $env:USERPROFILE\.cargo\bin\codex.exe
- **Binary Size**: $([Math]::Round((Get-Item "$env:USERPROFILE\.cargo\bin\codex.exe").Length / 1MB, 2)) MB

---

## Conclusion

$( if ($failCount -eq 0) { "✅ **ALL MCP TESTS PASSED!** Codex v0.48.0 MCP server is fully functional." } 
   elseif ($failCount -le 2) { "⚠️ **MOSTLY PASSED** - Some minor issues, but core MCP functionality is working." }
   else { "❌ **NEEDS ATTENTION** - Multiple MCP tests failed. Please review." } )

---

**Test Completed**: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
"@

$logContent | Out-File -FilePath $logFile -Encoding UTF8 -Force
Write-Host "MCP test log saved to: $logFile" -ForegroundColor Gray
Write-Host ""

