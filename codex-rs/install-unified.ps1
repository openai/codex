# Codex Unified Install Script
# Force install latest build with kernel driver option

param(
    [switch]$WithKernel,   # Install kernel driver
    [switch]$TestSign      # Use test signature
)

Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "  Codex Unified 強制インストール" -ForegroundColor Cyan
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Find MSI
$msiPath = ".\tauri-gui\src-tauri\target\release\bundle\msi"

if (-not (Test-Path $msiPath)) {
    Write-Host "❌ MSIが見つかりません。先にビルドしてください:" -ForegroundColor Red
    Write-Host "   .\build-unified.ps1 -Release" -ForegroundColor Yellow
    exit 1
}

$msi = Get-ChildItem "$msiPath\*.msi" | Sort-Object LastWriteTime -Descending | Select-Object -First 1

if (-not $msi) {
    Write-Host "❌ MSIファイルが見つかりません" -ForegroundColor Red
    exit 1
}

Write-Host "MSI: $($msi.Name)" -ForegroundColor Gray
Write-Host "Size: $([math]::Round($msi.Length / 1MB, 2)) MB" -ForegroundColor Gray
Write-Host ""

# Step 2: Uninstall existing
Write-Host "[1/3] 既存インストール削除..." -ForegroundColor Yellow

$existing = Get-WmiObject -Class Win32_Product | Where-Object { $_.Name -like "*Codex*" }
if ($existing) {
    Write-Host "  削除中: $($existing.Name)" -ForegroundColor Gray
    try {
        $existing.Uninstall() | Out-Null
        Write-Host "  ✅ 削除完了" -ForegroundColor Green
        Start-Sleep -Seconds 2
    } catch {
        Write-Host "  ⚠️  削除失敗（継続）" -ForegroundColor Yellow
    }
} else {
    Write-Host "  既存インストールなし" -ForegroundColor Gray
}

# Step 3: Install MSI
Write-Host ""
Write-Host "[2/3] MSIインストール..." -ForegroundColor Yellow

$msiFullPath = $msi.FullName

Start-Process -FilePath "msiexec.exe" -ArgumentList "/i", "`"$msiFullPath`"", "/qb", "REINSTALL=ALL", "REINSTALLMODE=vomus" -Wait

if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ インストール完了" -ForegroundColor Green
} else {
    Write-Host "  ❌ インストール失敗（コード: $LASTEXITCODE）" -ForegroundColor Red
    exit 1
}

# Step 4: Kernel driver (optional)
if ($WithKernel) {
    Write-Host ""
    Write-Host "[3/3] カーネルドライバーインストール..." -ForegroundColor Yellow
    
    if ($TestSign) {
        Write-Host "  テスト署名モード有効化..." -ForegroundColor Cyan
        bcdedit /set testsigning on
        Write-Host "  ⚠️  再起動が必要です" -ForegroundColor Yellow
    }
    
    $driverPath = ".\kernel-extensions\windows\ai_driver"
    
    if (Test-Path "$driverPath\ai_driver.inf") {
        Write-Host "  ドライバーインストール中..." -ForegroundColor Cyan
        pnputil /add-driver "$driverPath\ai_driver.inf" /install
        
        Write-Host "  サービス開始..." -ForegroundColor Cyan
        sc start AiDriver
        
        Write-Host "  ✅ カーネルドライバーインストール完了" -ForegroundColor Green
    } else {
        Write-Host "  ❌ ドライバーファイルが見つかりません" -ForegroundColor Red
    }
} else {
    Write-Host ""
    Write-Host "[3/3] カーネルドライバースキップ" -ForegroundColor Yellow
    Write-Host "  カーネル機能を使用する場合:" -ForegroundColor Gray
    Write-Host "  .\install-unified.ps1 -WithKernel -TestSign" -ForegroundColor Gray
}

# Summary
Write-Host ""
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "  🎉 インストール完了！" -ForegroundColor Green
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "インストール内容:" -ForegroundColor White
Write-Host "  ✅ Codex Tauri GUI（システムトレイ常駐）" -ForegroundColor Gray
Write-Host "  ✅ ファイル監視機能" -ForegroundColor Gray
Write-Host "  ✅ VR/AR Git可視化（4D）" -ForegroundColor Gray
Write-Host "  ✅ Codex Core統合" -ForegroundColor Gray
if ($WithKernel) {
    Write-Host "  ✅ カーネルドライバー（AIネイティブOS）" -ForegroundColor Gray
}
Write-Host ""
Write-Host "システムトレイのCodexアイコンから起動してください" -ForegroundColor Cyan
Write-Host ""

# Play sound
Add-Type -AssemblyName System.Windows.Forms
$player = New-Object System.Media.SoundPlayer "C:\Users\downl\Desktop\SO8T\.cursor\marisa_owattaze.wav"
$player.PlaySync()

Write-Host "Owattaze!" -ForegroundColor Magenta

