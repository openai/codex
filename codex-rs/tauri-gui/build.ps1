# Codex Tauri Build Script for Windows
# This script builds the Tauri application and creates MSI installer

param(
    [switch]$Release,
    [switch]$Dev
)

Write-Host "🚀 Codex Tauri Build Script" -ForegroundColor Cyan
Write-Host "=============================" -ForegroundColor Cyan
Write-Host ""

# Check Node.js
Write-Host "✅ Checking Node.js..." -ForegroundColor Green
try {
    $nodeVersion = node --version
    Write-Host "   Node.js version: $nodeVersion" -ForegroundColor Gray
} catch {
    Write-Host "❌ Node.js not found. Please install Node.js 18+." -ForegroundColor Red
    exit 1
}

# Check Rust
Write-Host "✅ Checking Rust..." -ForegroundColor Green
try {
    $rustVersion = rustc --version
    Write-Host "   Rust version: $rustVersion" -ForegroundColor Gray
} catch {
    Write-Host "❌ Rust not found. Please install Rust 1.70+." -ForegroundColor Red
    exit 1
}

# Install npm dependencies
Write-Host ""
Write-Host "📦 Installing npm dependencies..." -ForegroundColor Yellow
npm install
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ npm install failed" -ForegroundColor Red
    exit 1
}

if ($Dev) {
    # Run in development mode
    Write-Host ""
    Write-Host "🔧 Starting development server..." -ForegroundColor Yellow
    npm run tauri:dev
} else {
    # Build for production
    Write-Host ""
    Write-Host "🔨 Building Tauri application..." -ForegroundColor Yellow
    
    if ($Release) {
        npm run tauri build
    } else {
        npm run tauri build
    }
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Build failed" -ForegroundColor Red
        exit 1
    }
    
    Write-Host ""
    Write-Host "✨ Build completed successfully!" -ForegroundColor Green
    Write-Host ""
    Write-Host "📦 MSI Installer location:" -ForegroundColor Cyan
    Write-Host "   src-tauri\target\release\bundle\msi\" -ForegroundColor Gray
    Write-Host ""
    Write-Host "🎉 Ready to distribute!" -ForegroundColor Green
}

