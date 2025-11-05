# Codex Unified Build Script
# Builds tauri-gui with VR/AR + Kernel integration

param(
    [switch]$Release,
    [switch]$Dev,
    [switch]$Fast,     # Skip frontend build
    [switch]$Verbose
)

$ErrorActionPreference = "Continue"

Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "  Codex Unified VR/AR AIネイティブOS ビルド" -ForegroundColor Cyan
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""

$startTime = Get-Date

# Build mode
$buildMode = if ($Release -or (-not $Dev)) { "release" } else { "debug" }
Write-Host "ビルドモード: $buildMode" -ForegroundColor Yellow
Write-Host ""

# Check sccache
if (Get-Command sccache -ErrorAction SilentlyContinue) {
    $env:RUSTC_WRAPPER = "sccache"
    Write-Host "✅ sccache有効化（差分ビルド高速化）" -ForegroundColor Green
} else {
    Write-Host "⚠️  sccache未インストール（推奨）" -ForegroundColor Yellow
}

# Navigate to tauri-gui
cd tauri-gui

# Step 1: Frontend build
if (-not $Fast) {
    Write-Host ""
    Write-Host "[1/3] 📦 Frontend Build（Vite + React）" -ForegroundColor Cyan
    Write-Host "─────────────────────────────────────" -ForegroundColor Gray
    
    npm install 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ npm install failed" -ForegroundColor Red
        cd ..
        exit 1
    }
    
    npm run build 2>&1 | Select-String -Pattern "✓|error|warn" | ForEach-Object {
        Write-Host "  $_" -ForegroundColor Gray
    }
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Frontend build failed" -ForegroundColor Red
        cd ..
        exit 1
    }
    
    Write-Host "✅ Frontend build complete" -ForegroundColor Green
} else {
    Write-Host "[1/3] ⏭️  Frontend build skipped (--Fast mode)" -ForegroundColor Yellow
}

# Step 2: Rust build (with progress)
Write-Host ""
Write-Host "[2/3] 🦀 Rust Build（差分ビルド）" -ForegroundColor Cyan
Write-Host "─────────────────────────────────────" -ForegroundColor Gray

cd src-tauri

$cargoStart = Get-Date

if ($buildMode -eq "release") {
    cargo build --release 2>&1 | ForEach-Object {
        if ($_ -match "Compiling|Finished|error") {
            if ($_ -match "Compiling") {
                Write-Host "  🔨 $_" -ForegroundColor Cyan
            } elseif ($_ -match "Finished") {
                Write-Host "  ✅ $_" -ForegroundColor Green
            } else {
                Write-Host "  ❌ $_" -ForegroundColor Red
            }
        }
    }
} else {
    cargo build 2>&1 | ForEach-Object {
        if ($_ -match "Compiling|Finished|error") {
            if ($_ -match "Compiling") {
                Write-Host "  🔨 $_" -ForegroundColor Cyan
            } elseif ($_ -match "Finished") {
                Write-Host "  ✅ $_" -ForegroundColor Green
            } else {
                Write-Host "  ❌ $_" -ForegroundColor Red
            }
        }
    }
}

$cargoTime = (Get-Date) - $cargoStart

if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Rust build failed" -ForegroundColor Red
    cd ../..
    exit 1
}

Write-Host ""
Write-Host "✅ Rust build complete（所要時間: $([math]::Round($cargoTime.TotalSeconds, 1))秒）" -ForegroundColor Green

$exePath = ".\target\$buildMode\codex-tauri-gui.exe"
if (Test-Path $exePath) {
    $exeSize = [math]::Round((Get-Item $exePath).Length / 1MB, 2)
    Write-Host "   ファイル: codex-tauri-gui.exe" -ForegroundColor Gray
    Write-Host "   サイズ: $exeSize MB" -ForegroundColor Gray
}

cd ..

# Step 3: MSI Bundle (Release only)
if ($buildMode -eq "release") {
    Write-Host ""
    Write-Host "[3/3] 📦 MSI Installer作成" -ForegroundColor Cyan
    Write-Host "─────────────────────────────────────" -ForegroundColor Gray
    
    npx tauri build 2>&1 | Select-String -Pattern "Finished|Creating|error" | ForEach-Object {
        Write-Host "  $_" -ForegroundColor Gray
    }
    
    if ($LASTEXITCODE -eq 0) {
        $msiPath = ".\src-tauri\target\release\bundle\msi"
        if (Test-Path $msiPath) {
            $msi = Get-ChildItem "$msiPath\*.msi" | Select-Object -First 1
            if ($msi) {
                $msiSize = [math]::Round($msi.Length / 1MB, 2)
                Write-Host ""
                Write-Host "✅ MSI作成完了" -ForegroundColor Green
                Write-Host "   ファイル: $($msi.Name)" -ForegroundColor Gray
                Write-Host "   サイズ: $msiSize MB" -ForegroundColor Gray
                Write-Host "   パス: $($msi.FullName)" -ForegroundColor Gray
            }
        }
    }
} else {
    Write-Host ""
    Write-Host "[3/3] ⏭️  MSI作成スキップ（Debug mode）" -ForegroundColor Yellow
}

# Build summary
$totalTime = (Get-Date) - $startTime

Write-Host ""
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "  ✨ ビルド完了！" -ForegroundColor Green
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "総ビルド時間: $([math]::Round($totalTime.TotalMinutes, 1))分" -ForegroundColor White
Write-Host "Rustビルド: $([math]::Round($cargoTime.TotalSeconds, 1))秒" -ForegroundColor White
Write-Host ""

if ($buildMode -eq "release") {
    Write-Host "次のステップ:" -ForegroundColor Cyan
    Write-Host "  1. インストール: ..\install-unified.ps1" -ForegroundColor Gray
    Write-Host "  2. セキュリティテスト: ..\test-security-unified.ps1" -ForegroundColor Gray
} else {
    Write-Host "次のステップ:" -ForegroundColor Cyan
    Write-Host "  1. 実行: .\src-tauri\target\debug\codex-tauri-gui.exe" -ForegroundColor Gray
}

cd ..

# Play completion sound
Write-Host ""
Write-Host "🔊 完了音声再生..." -ForegroundColor Magenta

Add-Type -AssemblyName System.Windows.Forms
$player = New-Object System.Media.SoundPlayer "C:\Users\downl\Desktop\SO8T\.cursor\marisa_owattaze.wav"
$player.PlaySync()

Write-Host ""
Write-Host "Owattaze!" -ForegroundColor Magenta

