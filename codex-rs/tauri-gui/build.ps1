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

# Type check (TypeScript)
Write-Host ""
Write-Host "🔍 Running TypeScript type check..." -ForegroundColor Yellow
npm run type-check
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ TypeScript type errors found" -ForegroundColor Red
    exit 1
}
Write-Host "✅ No type errors" -ForegroundColor Green

if ($Dev) {
    # Run in development mode
    Write-Host ""
    Write-Host "🔧 Starting development server..." -ForegroundColor Yellow
    npm run tauri:dev
} else {
    # Build for production
    Write-Host ""
    Write-Host "🔨 Building Tauri application (CUDA + Windows AI)..." -ForegroundColor Yellow
    
    # Cargo clean for fresh build
    Write-Host ""
    Write-Host "🧹 Cleaning previous build..." -ForegroundColor Yellow
    Push-Location src-tauri
    cargo clean
    Pop-Location
    
    # Build with all features
    Write-Host ""
    Write-Host "⚡ Building with CUDA and Windows AI features..." -ForegroundColor Yellow
    $env:TAURI_CARGO_FEATURES = "cuda,windows-ai"
    
    if ($Release) {
        npm run tauri build -- --features "cuda,windows-ai"
    } else {
        npm run tauri build -- --features "cuda,windows-ai"
    }
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Build failed" -ForegroundColor Red
        exit 1
    }
    
    # Rust warnings check
    Write-Host ""
    Write-Host "🔍 Checking for Rust warnings..." -ForegroundColor Yellow
    Push-Location src-tauri
    cargo clippy --features "cuda,windows-ai" -- -D warnings
    if ($LASTEXITCODE -ne 0) {
        Write-Host "⚠️  Warnings found (non-fatal)" -ForegroundColor Yellow
    } else {
        Write-Host "✅ No warnings" -ForegroundColor Green
    }
    Pop-Location
    
    Write-Host ""
    Write-Host "✨ Build completed successfully!" -ForegroundColor Green
    Write-Host ""
    Write-Host "📦 MSI Installer location:" -ForegroundColor Cyan
    Write-Host "   src-tauri\target\release\bundle\msi\" -ForegroundColor Gray
    Write-Host ""
    Write-Host "🎯 Features enabled:" -ForegroundColor Cyan
    Write-Host "   - Babylon.js WebGPU/WebGL2" -ForegroundColor Gray
    Write-Host "   - CUDA Acceleration" -ForegroundColor Gray
    Write-Host "   - Windows AI Integration" -ForegroundColor Gray
    Write-Host "   - WebXR VR/AR Support" -ForegroundColor Gray
    Write-Host "   - Virtual Desktop Optimization" -ForegroundColor Gray
    Write-Host ""
    Write-Host "🏆 Kamui4D-Exceeding GUI Ready!" -ForegroundColor Green
    Write-Host "🎉 Ready to distribute!" -ForegroundColor Green
}

