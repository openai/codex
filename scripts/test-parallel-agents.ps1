# Codex Sub-Agent Parallel Execution Test
# Version: 0.56.0
# Author: zapabob

param(
    [string[]]$Agents = @("code-reviewer", "test-gen", "sec-audit"),
    [string]$TestDir = "test-workspace",
    [int]$Timeout = 300,
    [switch]$Verbose
)

$ErrorActionPreference = "Stop"

Write-Host "🧪 Codex Sub-Agent Parallel Execution Test" -ForegroundColor Cyan
Write-Host "===========================================" -ForegroundColor Cyan

# Create test workspace
if (-not (Test-Path $TestDir)) {
    New-Item -ItemType Directory -Path $TestDir -Force | Out-Null
    Write-Host "✅ Created test workspace: $TestDir" -ForegroundColor Green
}

# Create test file
$testFile = Join-Path $TestDir "test.rs"
$testCode = @"
// Test code for parallel agent execution
use std::collections::HashMap;

pub struct UserManager {
    users: HashMap<String, String>,
}

impl UserManager {
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
        }
    }
    
    // SECURITY: This function is vulnerable to injection attacks
    pub fn add_user(&mut self, username: String, password: String) {
        // TODO: Add password validation
        // TODO: Add unit tests
        self.users.insert(username, password);
    }
    
    pub fn authenticate(&self, username: &str, password: &str) -> bool {
        self.users.get(username)
            .map(|p| p == password)
            .unwrap_or(false)
    }
}
"@

Set-Content -Path $testFile -Value $testCode -Encoding UTF8
Write-Host "✅ Created test file: $testFile" -ForegroundColor Green
Write-Host ""

# Prepare agent tasks
$tasks = @()
foreach ($agent in $Agents) {
    $tasks += @{
        Agent = $agent
        Command = "codex delegate $agent --scope $TestDir"
        StartTime = $null
        EndTime = $null
        Duration = $null
        ExitCode = $null
        Output = @()
    }
}

Write-Host "📋 Test Plan:" -ForegroundColor Yellow
Write-Host "  ├─ Agents: $($Agents -join ', ')" -ForegroundColor White
Write-Host "  ├─ Test Dir: $TestDir" -ForegroundColor White
Write-Host "  ├─ Timeout: ${Timeout}s" -ForegroundColor White
Write-Host "  └─ Parallel Execution: $($tasks.Count) agents" -ForegroundColor White
Write-Host ""

# Start parallel execution
Write-Host "🚀 Starting parallel agent execution..." -ForegroundColor Cyan
$jobs = @()

foreach ($task in $tasks) {
    Write-Host "  ▶️  Starting: $($task.Agent)" -ForegroundColor Yellow
    
    $scriptBlock = {
        param($command, $verbose)
        
        $output = @()
        $startTime = Get-Date
        
        try {
            if ($verbose) {
                $result = Invoke-Expression $command 2>&1
            } else {
                $result = Invoke-Expression $command 2>&1 | Out-String
            }
            
            $output += $result
            $exitCode = $LASTEXITCODE
        } catch {
            $output += "ERROR: $($_.Exception.Message)"
            $exitCode = 1
        }
        
        $endTime = Get-Date
        
        return @{
            StartTime = $startTime
            EndTime = $endTime
            Duration = ($endTime - $startTime).TotalSeconds
            ExitCode = $exitCode
            Output = $output
        }
    }
    
    $job = Start-Job -ScriptBlock $scriptBlock -ArgumentList $task.Command, $Verbose
    $jobs += @{
        Job = $job
        Task = $task
    }
    
    $task.StartTime = Get-Date
}

Write-Host "✅ All agents started" -ForegroundColor Green
Write-Host ""

# Monitor progress
Write-Host "⏳ Monitoring execution (timeout: ${Timeout}s)..." -ForegroundColor Cyan

$startTime = Get-Date
$completed = 0

while ($completed -lt $jobs.Count) {
    $elapsed = ((Get-Date) - $startTime).TotalSeconds
    
    if ($elapsed -gt $Timeout) {
        Write-Host "⚠️  Timeout reached! Stopping remaining jobs..." -ForegroundColor Yellow
        foreach ($jobInfo in $jobs) {
            if ($jobInfo.Job.State -eq "Running") {
                Stop-Job -Job $jobInfo.Job
                $jobInfo.Task.ExitCode = -1
                $jobInfo.Task.Output += "TIMEOUT"
            }
        }
        break
    }
    
    $completed = 0
    foreach ($jobInfo in $jobs) {
        if ($jobInfo.Job.State -ne "Running") {
            $completed++
        }
    }
    
    Write-Progress -Activity "Parallel Agent Execution" `
                   -Status "$completed / $($jobs.Count) agents completed" `
                   -PercentComplete (($completed / $jobs.Count) * 100)
    
    Start-Sleep -Seconds 1
}

Write-Progress -Activity "Parallel Agent Execution" -Completed

# Collect results
Write-Host "📊 Collecting results..." -ForegroundColor Cyan

foreach ($jobInfo in $jobs) {
    $result = Receive-Job -Job $jobInfo.Job
    $jobInfo.Task.StartTime = $result.StartTime
    $jobInfo.Task.EndTime = $result.EndTime
    $jobInfo.Task.Duration = $result.Duration
    $jobInfo.Task.ExitCode = $result.ExitCode
    $jobInfo.Task.Output = $result.Output
    
    Remove-Job -Job $jobInfo.Job
}

# Display results
Write-Host ""
Write-Host "╔════════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║                     Test Results Summary                       ║" -ForegroundColor Green
Write-Host "╚════════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""

$successCount = 0
$failureCount = 0

foreach ($task in $tasks) {
    $status = if ($task.ExitCode -eq 0) {
        $successCount++
        "✅ SUCCESS"
    } else {
        $failureCount++
        "❌ FAILED"
    }
    
    Write-Host "  $status - $($task.Agent)" -ForegroundColor $(if ($task.ExitCode -eq 0) { "Green" } else { "Red" })
    Write-Host "    ├─ Duration: $([math]::Round($task.Duration, 2))s" -ForegroundColor Gray
    Write-Host "    ├─ Exit Code: $($task.ExitCode)" -ForegroundColor Gray
    
    if ($Verbose -and $task.Output) {
        Write-Host "    └─ Output:" -ForegroundColor Gray
        foreach ($line in $task.Output) {
            Write-Host "       $line" -ForegroundColor DarkGray
        }
    }
    Write-Host ""
}

# Final summary
$totalDuration = ($tasks | Measure-Object -Property Duration -Maximum).Maximum

Write-Host "╔════════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║                     Performance Metrics                        ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""
Write-Host "  📊 Total Agents:      $($tasks.Count)" -ForegroundColor White
Write-Host "  ✅ Successful:        $successCount" -ForegroundColor Green
Write-Host "  ❌ Failed:            $failureCount" -ForegroundColor Red
Write-Host "  ⏱️  Max Duration:      $([math]::Round($totalDuration, 2))s" -ForegroundColor White
Write-Host "  ⚡ Parallel Speedup:  $(if ($totalDuration -gt 0) { [math]::Round(($tasks | Measure-Object -Property Duration -Sum).Sum / $totalDuration, 2) } else { 'N/A' })x" -ForegroundColor Yellow
Write-Host ""

# Cleanup
if (-not $Verbose) {
    Remove-Item -Path $TestDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "🧹 Cleaned up test workspace" -ForegroundColor Gray
}

# Exit code
if ($failureCount -eq 0) {
    Write-Host "🎉 All agents completed successfully!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "⚠️  Some agents failed. Check output above for details." -ForegroundColor Yellow
    exit 1
}


