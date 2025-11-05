# sccache Windows Stable Install Script
# Run: .\install-sccache-simple.ps1
# Uses pre-built binary from GitHub releases (stable version)

Write-Host "Installing sccache (Windows stable version)..." -ForegroundColor Cyan

# Check if sccache is already installed
$sccache = Get-Command sccache -ErrorAction SilentlyContinue

if ($null -eq $sccache) {
    Write-Host "Downloading sccache stable binary from GitHub..." -ForegroundColor Yellow
    
    # Use stable version v0.8.2 (last known stable for Windows)
    $version = "v0.8.2"
    $url = "https://github.com/mozilla/sccache/releases/download/$version/sccache-$version-x86_64-pc-windows-msvc.tar.gz"
    $downloadPath = "$env:TEMP\sccache.tar.gz"
    $extractPath = "$env:TEMP\sccache-extract"
    $installPath = "$env:USERPROFILE\.cargo\bin"
    
    try {
        # Download
        Write-Host "  Downloading from: $url" -ForegroundColor Gray
        Invoke-WebRequest -Uri $url -OutFile $downloadPath -UseBasicParsing
        
        # Extract using tar (built-in Windows 10+)
        Write-Host "  Extracting archive..." -ForegroundColor Gray
        if (Test-Path $extractPath) {
            Remove-Item $extractPath -Recurse -Force
        }
        New-Item -ItemType Directory -Path $extractPath -Force | Out-Null
        tar -xzf $downloadPath -C $extractPath
        
        # Copy to cargo bin directory
        Write-Host "  Installing to: $installPath" -ForegroundColor Gray
        if (-not (Test-Path $installPath)) {
            New-Item -ItemType Directory -Path $installPath -Force | Out-Null
        }
        
        $exePath = Get-ChildItem -Path $extractPath -Recurse -Filter "sccache.exe" | Select-Object -First 1
        if ($null -eq $exePath) {
            throw "sccache.exe not found in archive"
        }
        
        Copy-Item $exePath.FullName -Destination "$installPath\sccache.exe" -Force
        
        # Cleanup
        Remove-Item $downloadPath -Force -ErrorAction SilentlyContinue
        Remove-Item $extractPath -Recurse -Force -ErrorAction SilentlyContinue
        
        Write-Host "  sccache v0.8.2 installed successfully!" -ForegroundColor Green
    }
    catch {
        Write-Host "  Failed to install sccache: $_" -ForegroundColor Red
        Write-Host "  Trying alternative: scoop install sccache" -ForegroundColor Yellow
        
        # Fallback to scoop if available
        $scoop = Get-Command scoop -ErrorAction SilentlyContinue
        if ($null -ne $scoop) {
            scoop install sccache
        } else {
            Write-Host "  Please install manually from: https://github.com/mozilla/sccache/releases" -ForegroundColor Red
            exit 1
        }
    }
} else {
    Write-Host "sccache is already installed: $($sccache.Source)" -ForegroundColor Green
    Write-Host "Version: " -NoNewline
    & sccache --version
}

# Set environment variable for current session
$env:RUSTC_WRAPPER = "sccache"
Write-Host "Environment variable set for current session" -ForegroundColor Green

# Add to PowerShell profile
$profileContent = '$env:RUSTC_WRAPPER = "sccache"'

if (Test-Path $PROFILE) {
    $existing = Get-Content $PROFILE -Raw -ErrorAction SilentlyContinue
    if ($null -eq $existing -or $existing -notmatch "RUSTC_WRAPPER") {
        Add-Content -Path $PROFILE -Value "`n# sccache for Rust builds`n$profileContent"
        Write-Host "Added to PowerShell profile: $PROFILE" -ForegroundColor Green
    } else {
        Write-Host "Already configured in PowerShell profile" -ForegroundColor Yellow
    }
} else {
    $profileDir = Split-Path $PROFILE -Parent
    if (-not (Test-Path $profileDir)) {
        New-Item -ItemType Directory -Path $profileDir -Force | Out-Null
    }
    Set-Content -Path $PROFILE -Value "# sccache for Rust builds`n$profileContent"
    Write-Host "Created PowerShell profile: $PROFILE" -ForegroundColor Green
}

# Show statistics
Write-Host "`nCurrent sccache statistics:" -ForegroundColor Cyan
sccache --show-stats

Write-Host "`nUsage:" -ForegroundColor Cyan
Write-Host "  Build: cargo build --release -p codex-cli" -ForegroundColor White
Write-Host "  Stats: sccache --show-stats" -ForegroundColor White
Write-Host "  Clear: sccache --zero-stats" -ForegroundColor White
Write-Host "`nNote: Using stable v0.8.2 for Windows compatibility" -ForegroundColor Yellow
Write-Host "Second build will be 70-90% faster!" -ForegroundColor Green

