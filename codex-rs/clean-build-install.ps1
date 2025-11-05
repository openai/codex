# Codex Clean Release Build & Global Install with Error Recovery
# 
# Usage:
#   .\clean-build-install.ps1
#   .\clean-build-install.ps1 -SkipClean  # Skip clean step
#   .\clean-build-install.ps1 -Verbose    # Verbose logging

param(
    [switch]$SkipClean = $false,
    [switch]$Verbose = $false
)

$ErrorActionPreference = "Continue"
$ProgressPreference = 'SilentlyContinue'

# Color output functions
function Write-Status {
    param([string]$Message, [string]$Color = "Cyan")
    Write-Host "[*] $Message" -ForegroundColor $Color
}

function Write-Success {
    param([string]$Message)
    Write-Host "[OK] $Message" -ForegroundColor Green
}

function Write-ErrorMsg {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

function Write-WarningMsg {
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

# Log file
$LogFile = "clean-build-install.log"
$Timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"

function Log {
    param([string]$Message)
    $Entry = "[$Timestamp] $Message"
    Add-Content -Path $LogFile -Value $Entry -Encoding UTF8
    if ($Verbose) {
        Write-Host $Entry -ForegroundColor Gray
    }
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Codex Clean Build & Global Install" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

Log "=== Build started ==="

# Step 0: Auto-detect codex-rs directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$CurrentDir = Get-Location

# Check if we're already in codex-rs
if (-not (Test-Path "Cargo.toml")) {
    Write-Status "Auto-detecting codex-rs directory..."
    
    # Try relative paths
    $PossiblePaths = @(
        $ScriptDir,
        (Join-Path $CurrentDir "codex-rs"),
        (Join-Path (Split-Path $CurrentDir -Parent) "codex-rs"),
        "C:\Users\downl\Desktop\codex-main\codex-main\codex-rs"
    )
    
    $Found = $false
    foreach ($Path in $PossiblePaths) {
        if (Test-Path (Join-Path $Path "Cargo.toml")) {
            Write-Status "Found codex-rs at: $Path"
            Set-Location $Path
            $Found = $true
            break
        }
    }
    
    if (-not $Found) {
        Write-ErrorMsg "Could not find codex-rs directory with Cargo.toml"
        Write-Host "Tried paths:" -ForegroundColor Yellow
        foreach ($Path in $PossiblePaths) {
            Write-Host "  - $Path" -ForegroundColor Gray
        }
        Write-Host "`nPlease run from codex-rs directory or parent directory" -ForegroundColor Yellow
        exit 1
    }
}

# Step 1: Workspace validation
Write-Status "Step 1/7: Validating workspace..."
if (-not (Test-Path "Cargo.toml")) {
    Write-ErrorMsg "Cargo.toml not found after directory detection."
    exit 1
}
Write-Success "Workspace validated: $(Get-Location)"
Log "Workspace validated: $(Get-Location)"

# Step 2: Clean build (optional)
if (-not $SkipClean) {
    Write-Status "Step 2/7: Cleaning build artifacts (cargo clean)..."
    Log "Running cargo clean"
    
    cargo clean 2>&1 | Out-String | ForEach-Object { Log $_ }
    
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Clean completed"
    } else {
        Write-WarningMsg "cargo clean failed, but continuing"
    }
} else {
    Write-Status "Step 2/7: Skipping clean step"
}

# Step 3: Format
Write-Status "Step 3/7: Formatting code (just fmt)..."
Log "Running just fmt"

if (Get-Command just -ErrorAction SilentlyContinue) {
    just fmt 2>&1 | Out-String | ForEach-Object { Log $_ }
    Write-Success "Format completed"
} else {
    Write-WarningMsg "just command not found. Using cargo fmt"
    cargo fmt --all 2>&1 | Out-String | ForEach-Object { Log $_ }
}

# Step 4: Release build
Write-Status "Step 4/7: Building release (codex-cli)..."
Write-Host "   [INFO] This may take several minutes..." -ForegroundColor Yellow
Log "Running cargo build --release -p codex-cli"

$BuildStart = Get-Date
$BuildOutput = cargo build --release -p codex-cli 2>&1 | Out-String
$BuildDuration = (Get-Date) - $BuildStart
Log $BuildOutput

if ($LASTEXITCODE -eq 0) {
    Write-Success "Build succeeded!"
} else {
    Write-ErrorMsg "Build failed"
    Write-Host $BuildOutput -ForegroundColor Red
    
    # Special handling for ring crate
    if ($BuildOutput -match "ring") {
        Write-WarningMsg "Detected ring crate build error"
        Write-Status "Attempting workaround: check for existing binary"
        
        if (Test-Path ".\target\release\codex.exe") {
            Write-Status "Found existing binary. Using it"
        } else {
            Write-ErrorMsg "Cannot continue build"
            Write-Host "`nSuggested fix:" -ForegroundColor Yellow
            Write-Host "  1. Install Visual Studio Build Tools" -ForegroundColor White
            Write-Host "  2. Run: cargo update -p ring" -ForegroundColor White
            Write-Host "  3. Re-run this script" -ForegroundColor White
            exit 1
        }
    } else {
        exit 1
    }
}

# Step 5: Verify binary
Write-Status "Step 5/7: Verifying built binary..."
$BinaryPath = ".\target\release\codex.exe"

if (Test-Path $BinaryPath) {
    $FileInfo = Get-Item $BinaryPath
    Write-Success "Binary verified"
    Write-Host "   Size: $([math]::Round($FileInfo.Length / 1MB, 2)) MB" -ForegroundColor Gray
    Write-Host "   Modified: $($FileInfo.LastWriteTime)" -ForegroundColor Gray
    Log "Binary found: $BinaryPath ($($FileInfo.Length) bytes)"
} else {
    Write-ErrorMsg "Binary not found: $BinaryPath"
    exit 1
}

# Step 6: Global install
Write-Status "Step 6/7: Installing globally..."
$InstallPath = "$env:USERPROFILE\.cargo\bin\codex.exe"

# Stop existing processes
Write-Status "   Checking for running codex processes..."
$CodexProcesses = Get-Process codex -ErrorAction SilentlyContinue
if ($CodexProcesses) {
    Write-WarningMsg "Found running codex process. Stopping..."
    $CodexProcesses | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Write-Success "Process stopped"
    Log "Stopped codex processes"
}

# Create backup
if (Test-Path $InstallPath) {
    $BackupPath = "$InstallPath.backup-$(Get-Date -Format 'yyyyMMdd-HHmmss')"
    Write-Status "   Backing up existing binary..."
    Copy-Item $InstallPath $BackupPath -Force -ErrorAction SilentlyContinue
    Write-Success "Backup created: $(Split-Path $BackupPath -Leaf)"
    Log "Backed up to $BackupPath"
}

# Install with retry
Write-Status "   Copying binary..."
$MaxRetries = 3
$RetryCount = 0
$InstallSuccess = $false

while ($RetryCount -lt $MaxRetries -and -not $InstallSuccess) {
    try {
        if ($RetryCount -gt 0) {
            Write-Status "   Retry $RetryCount/$MaxRetries..."
            Get-Process codex -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 3
        }
        
        Copy-Item $BinaryPath $InstallPath -Force
        $InstallSuccess = $true
        Write-Success "Installation completed!"
        Log "Installed to $InstallPath"
    } catch {
        $RetryCount++
        if ($RetryCount -lt $MaxRetries) {
            Write-WarningMsg "Installation failed. Retrying..."
            Start-Sleep -Seconds 2
        } else {
            Write-ErrorMsg "Installation failed after $MaxRetries retries: $_"
            Write-Host "`nManual recovery steps:" -ForegroundColor Yellow
            Write-Host "  1. Stop all codex.exe processes in Task Manager" -ForegroundColor White
            Write-Host "  2. Run:" -ForegroundColor White
            Write-Host "     Remove-Item $InstallPath -Force" -ForegroundColor Cyan
            Write-Host "     Copy-Item $BinaryPath $InstallPath -Force" -ForegroundColor Cyan
            Log "Installation failed after $MaxRetries retries: $_"
            exit 1
        }
    }
}

# Step 7: Verify installation
Write-Status "Step 7/7: Verifying installation..."
Start-Sleep -Seconds 1

$VersionOutput = & codex --version 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Success "Installation verified"
    Write-Host "   Version: $VersionOutput" -ForegroundColor Green
    Log "Version check: $VersionOutput"
} else {
    Write-ErrorMsg "codex command failed"
    Write-Host "   Error: $VersionOutput" -ForegroundColor Red
    Log "Version check failed: $VersionOutput"
    exit 1
}

# Final summary
Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  Build & Install Completed!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Install path: $InstallPath" -ForegroundColor Cyan
Write-Host "Log file: $LogFile" -ForegroundColor Cyan
Write-Host "Build time: $([math]::Round($BuildDuration.TotalMinutes, 1)) minutes" -ForegroundColor Cyan

# Backup list
$AllBackups = Get-ChildItem "$env:USERPROFILE\.cargo\bin\codex.exe.backup-*" -ErrorAction SilentlyContinue
if ($AllBackups) {
    Write-Host "`nAvailable backups: $($AllBackups.Count)" -ForegroundColor Gray
    $AllBackups | Select-Object -First 3 | ForEach-Object {
        Write-Host "  - $($_.Name) ($([math]::Round($_.Length / 1MB, 2)) MB)" -ForegroundColor Gray
    }
}

Write-Host "`nNext steps:" -ForegroundColor Yellow
Write-Host "  codex delegate code-reviewer --scope codex-rs\cli" -ForegroundColor White
Write-Host "  codex research 'Rust async patterns' --depth 3" -ForegroundColor White
Write-Host ""

Log "=== Build completed ==="
