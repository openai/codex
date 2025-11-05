# Codex Clean Release Build - Progress Display
# Created: 2025-10-15

param(
    [string]$Package = "codex-cli"
)

function Show-Progress {
    param(
        [string]$Activity,
        [int]$PercentComplete,
        [string]$Status
    )
    
    $barLength = 50
    $completed = [math]::Floor($barLength * $PercentComplete / 100)
    $remaining = $barLength - $completed
    
    $bar = "#" * $completed + "-" * $remaining
    
    Write-Host -NoNewline "`r$Activity [$bar] $PercentComplete% - $Status"
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  Codex Clean Release Build v0.48.0" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""

# Step 1: Environment Check
Write-Host "Step 1/5: Environment Check" -ForegroundColor Cyan
$rustVersion = cargo --version 2>&1 | Out-String
if ($LASTEXITCODE -eq 0) {
    Write-Host "  Rust: $($rustVersion.Trim())" -ForegroundColor Green
} else {
    Write-Host "  Error: Rust not found!" -ForegroundColor Red
    exit 1
}
Write-Host ""

# Step 2: Clean
Write-Host "Step 2/5: Clean Build Directory" -ForegroundColor Cyan
Push-Location codex-rs

Write-Host "  Running: cargo clean" -ForegroundColor White
cargo clean | Out-Null
Write-Host "  Status: Complete!" -ForegroundColor Green
Write-Host ""

# Step 3: Build with progress
Write-Host "Step 3/5: Release Build (this may take 2-3 minutes)" -ForegroundColor Cyan
Write-Host "  Package: $Package" -ForegroundColor White
Write-Host "  Target: release" -ForegroundColor White
Write-Host ""

$buildStart = Get-Date
$buildLog = "..\build-progress-$(Get-Date -Format 'yyyyMMdd-HHmmss').log"

# Start build job
$buildJob = Start-Job -ScriptBlock {
    param($pkg, $logFile, $workDir)
    Set-Location $workDir
    cargo build --release -p $pkg 2>&1 | Tee-Object -FilePath $logFile
} -ArgumentList $Package, $buildLog, (Get-Location).Path

# Progress monitoring
$lastCompilingCount = 0
$stageNames = @(
    "Downloading dependencies",
    "Compiling dependencies", 
    "Building core modules",
    "Building MCP server",
    "Building orchestration",
    "Building CLI",
    "Optimizing binary",
    "Finalizing"
)

while ($buildJob.State -eq 'Running') {
    Start-Sleep -Milliseconds 500
    
    if (Test-Path $buildLog) {
        $logLines = Get-Content $buildLog -ErrorAction SilentlyContinue
        $compilingCount = ($logLines | Select-String "Compiling" | Measure-Object).Count
        $finishedCount = ($logLines | Select-String "Finished" | Measure-Object).Count
        
        if ($finishedCount -gt 0) {
            $progress = 100
            $stage = "Complete!"
        } elseif ($compilingCount -gt $lastCompilingCount) {
            $lastCompilingCount = $compilingCount
            $progress = [math]::Min(95, $compilingCount * 2)
            $stageIndex = [math]::Min($stageNames.Length - 1, [math]::Floor($compilingCount / 10))
            $stage = $stageNames[$stageIndex]
        } else {
            $progress = [math]::Min(95, $lastCompilingCount * 2)
            $stageIndex = [math]::Min($stageNames.Length - 1, [math]::Floor($lastCompilingCount / 10))
            $stage = $stageNames[$stageIndex]
        }
        
        Show-Progress -Activity "Building" -PercentComplete $progress -Status $stage
    } else {
        Show-Progress -Activity "Building" -PercentComplete 5 -Status "Initializing..."
    }
}

# Wait for completion
$buildResult = Receive-Job $buildJob -Wait
$hasError = $buildResult | Select-String "error:" | Select-Object -First 1

Write-Host ""

if ($buildJob.State -eq 'Completed' -and -not $hasError) {
    $buildDuration = (Get-Date) - $buildStart
    Write-Host "  Status: Build succeeded!" -ForegroundColor Green
    Write-Host "  Time: $([math]::Round($buildDuration.TotalSeconds, 1))s" -ForegroundColor White
} else {
    Write-Host "  Status: Build failed!" -ForegroundColor Red
    Write-Host ""
    Write-Host "Last 15 lines of build log:" -ForegroundColor Yellow
    $buildResult | Select-Object -Last 15
    Remove-Job $buildJob
    Pop-Location
    exit 1
}

Remove-Job $buildJob
Write-Host ""

# Step 4: Verify Binary
Write-Host "Step 4/5: Verify Binary" -ForegroundColor Cyan
$binaryPath = "target\release\codex.exe"

if (Test-Path $binaryPath) {
    $fileSize = (Get-Item $binaryPath).Length / 1MB
    Write-Host "  Binary: $binaryPath" -ForegroundColor Green
    Write-Host "  Size: $([math]::Round($fileSize, 2)) MB" -ForegroundColor White
} else {
    Write-Host "  Error: Binary not found!" -ForegroundColor Red
    Pop-Location
    exit 1
}
Write-Host ""

# Step 5: Global Install
Write-Host "Step 5/5: Global Install" -ForegroundColor Cyan
Write-Host "  Running: cargo install --path cli --force" -ForegroundColor White

$installStart = Get-Date
cargo install --path cli --force 2>&1 | Out-Null
$installDuration = (Get-Date) - $installStart

if ($LASTEXITCODE -eq 0) {
    Write-Host "  Status: Install succeeded!" -ForegroundColor Green
    Write-Host "  Time: $([math]::Round($installDuration.TotalSeconds, 1))s" -ForegroundColor White
} else {
    Write-Host "  Status: Install failed!" -ForegroundColor Red
    Pop-Location
    exit 1
}

Pop-Location
Write-Host ""

# Final verification
Write-Host "========================================" -ForegroundColor Green
Write-Host "  VERIFICATION" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""

$version = codex --version 2>&1 | Out-String
Write-Host "Installed Version:" -ForegroundColor Yellow
Write-Host "  $($version.Trim())" -ForegroundColor White
Write-Host ""

# Summary
$totalDuration = (Get-Date) - $buildStart
Write-Host "Summary:" -ForegroundColor Yellow
Write-Host "  Version: 0.48.0" -ForegroundColor White
Write-Host "  Total Time: $([math]::Round($totalDuration.TotalSeconds, 1))s" -ForegroundColor White
Write-Host "  Binary Size: $([math]::Round($fileSize, 2)) MB" -ForegroundColor White
Write-Host "  Build Log: $buildLog" -ForegroundColor Gray
Write-Host ""
Write-Host "Status: READY TO USE!" -ForegroundColor Green -BackgroundColor Black
Write-Host ""
