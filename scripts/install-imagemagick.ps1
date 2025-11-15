# ImageMagick Installation Script
# Uses Chocolatey to install ImageMagick for SVG to ICO conversion

$ErrorActionPreference = "Stop"

Write-Host "Installing ImageMagick via Chocolatey..." -ForegroundColor Cyan

# Check if Chocolatey is installed
if (-not (Get-Command choco -ErrorAction SilentlyContinue)) {
    Write-Host "Error: Chocolatey is not installed." -ForegroundColor Red
    Write-Host "Please install Chocolatey from: https://chocolatey.org/install" -ForegroundColor Yellow
    exit 1
}

# Check if ImageMagick is already installed
$magickPath = Get-Command magick -ErrorAction SilentlyContinue
if ($magickPath) {
    Write-Host "ImageMagick is already installed: $($magickPath.Source)" -ForegroundColor Green
    & magick --version
    exit 0
}

# Install ImageMagick
Write-Host "Installing ImageMagick (this may take a few minutes)..." -ForegroundColor Yellow
try {
    choco install imagemagick -y --no-progress
    if ($LASTEXITCODE -eq 0) {
        Write-Host "ImageMagick installed successfully!" -ForegroundColor Green
        
        # Refresh environment variables
        $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
        
        # Verify installation
        Start-Sleep -Seconds 2
        $magickPath = Get-Command magick -ErrorAction SilentlyContinue
        if ($magickPath) {
            Write-Host "Verification: ImageMagick is available at: $($magickPath.Source)" -ForegroundColor Green
            & magick --version
        } else {
            Write-Host "Warning: ImageMagick installed but not found in PATH. Please restart your terminal." -ForegroundColor Yellow
        }
    } else {
        Write-Host "Error: ImageMagick installation failed." -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "Error installing ImageMagick: $_" -ForegroundColor Red
    exit 1
}

