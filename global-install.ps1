# Codex Global Installation Script
# Install Codex CLI globally on your system

Write-Host "=== Codex Global Installation ===" -ForegroundColor Green

$repoPath = "C:\Users\downl\Desktop\codex-main\codex-main"
Set-Location $repoPath

# Step 1: Build Rust binary (if not already built)
Write-Host "`nStep 1: Checking Rust binary..." -ForegroundColor Yellow

if (Test-Path "codex-rs/target/release/codex.exe") {
    Write-Host "Rust binary found!" -ForegroundColor Green
} else {
    Write-Host "Building Rust binary..." -ForegroundColor Yellow
    Set-Location codex-rs
    cargo build --release --bin codex
    Set-Location ..
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Build successful!" -ForegroundColor Green
    } else {
        Write-Host "Build failed!" -ForegroundColor Red
        exit 1
    }
}

# Step 2: Create bin directory in codex-cli
Write-Host "`nStep 2: Preparing npm package..." -ForegroundColor Yellow

$binDir = "codex-cli/bin"
if (-not (Test-Path $binDir)) {
    New-Item -ItemType Directory -Path $binDir | Out-Null
}

# Copy binary to bin directory
Copy-Item "codex-rs/target/release/codex.exe" "$binDir/codex.exe" -Force
Write-Host "Binary copied to $binDir" -ForegroundColor Green

# Step 3: Create npm package
Write-Host "`nStep 3: Creating npm package..." -ForegroundColor Yellow
Set-Location codex-cli

# Remove old tarball if exists
Remove-Item "*.tgz" -ErrorAction SilentlyContinue

npm pack 2>$null

if ($LASTEXITCODE -eq 0) {
    $tarball = Get-ChildItem "*.tgz" | Select-Object -First 1
    Write-Host "Package created: $($tarball.Name)" -ForegroundColor Green
} else {
    Write-Host "npm pack failed!" -ForegroundColor Red
    Set-Location ..
    exit 1
}

Set-Location ..

# Step 4: Global install
Write-Host "`nStep 4: Installing globally..." -ForegroundColor Yellow

$tarball = Get-ChildItem "codex-cli/*.tgz" | Select-Object -First 1
npm install -g $tarball.FullName 2>$null

if ($LASTEXITCODE -eq 0) {
    Write-Host "Global installation successful!" -ForegroundColor Green
} else {
    Write-Host "Installation failed!" -ForegroundColor Red
    Write-Host "Try running as Administrator" -ForegroundColor Yellow
    exit 1
}

# Step 5: Verify installation
Write-Host "`nStep 5: Verifying installation..." -ForegroundColor Yellow

$codexPath = (Get-Command codex -ErrorAction SilentlyContinue).Source
if ($codexPath) {
    Write-Host "Codex installed at: $codexPath" -ForegroundColor Green
    
    # Test version
    Write-Host "`nTesting codex command..." -ForegroundColor Yellow
    $version = codex --version 2>$null
    if ($version) {
        Write-Host "Version: $version" -ForegroundColor Cyan
    }
} else {
    Write-Host "Warning: codex command not found in PATH" -ForegroundColor Yellow
    Write-Host "You may need to restart your terminal" -ForegroundColor Yellow
}

# Summary
Write-Host "`n=== Installation Complete ===" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "Codex CLI has been installed globally!" -ForegroundColor Cyan
Write-Host "" -ForegroundColor Green
Write-Host "Usage:" -ForegroundColor Yellow
Write-Host "  codex --help                    # Show help" -ForegroundColor Cyan
Write-Host "  codex --version                 # Show version" -ForegroundColor Cyan
Write-Host "  codex chat                      # Start interactive chat" -ForegroundColor Cyan
Write-Host "" -ForegroundColor Green
Write-Host "If 'codex' command is not found:" -ForegroundColor Yellow
Write-Host "  1. Restart your terminal" -ForegroundColor Cyan
Write-Host "  2. Or run: refreshenv" -ForegroundColor Cyan
Write-Host "" -ForegroundColor Green
Write-Host "Installed version: 0.47.0-alpha.1" -ForegroundColor Green
Write-Host "All done!" -ForegroundColor Green

