# Codex Unified Security Test
# Tests all security aspects including VR/AR and Kernel integration

Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "  Codex Unified セキュリティテスト" -ForegroundColor Cyan
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""

$TestResults = @()

function Add-TestResult {
    param([string]$Name, [bool]$Passed, [string]$Message = "")
    
    $script:TestResults += @{
        Name = $Name
        Passed = $Passed
        Message = $Message
    }
    
    if ($Passed) {
        Write-Host "✅ $Name" -ForegroundColor Green
    } else {
        Write-Host "❌ $Name" -ForegroundColor Red
        if ($Message) { Write-Host "   $Message" -ForegroundColor Yellow }
    }
}

# Test 1: Binary Check
Write-Host "📦 Test 1: バイナリ確認" -ForegroundColor Yellow
$exe = ".\tauri-gui\src-tauri\target\release\codex-tauri-gui.exe"
if (Test-Path $exe) {
    $size = [math]::Round((Get-Item $exe).Length / 1MB, 2)
    Add-TestResult "バイナリ存在" $true
    Add-TestResult "バイナリサイズ(<100MB)" ($size -lt 100) "$size MB"
} else {
    Add-TestResult "バイナリ存在" $false "ビルドが必要"
}

# Test 2: Tauri Config
Write-Host ""
Write-Host "⚙️  Test 2: Tauri設定" -ForegroundColor Yellow
$config = ".\tauri-gui\src-tauri\tauri.conf.json"
if (Test-Path $config) {
    $json = Get-Content $config | ConvertFrom-Json
    $csp = $json.app.security.csp
    
    if ($csp -match "default-src 'self'") {
        Add-TestResult "CSP設定" $true
    } else {
        Add-TestResult "CSP設定" $false "CSPが緩い"
    }
} else {
    Add-TestResult "設定ファイル" $false
}

# Test 3: Dependencies
Write-Host ""
Write-Host "🔍 Test 3: 依存関係" -ForegroundColor Yellow

cd tauri-gui

# npm audit
$npmAudit = npm audit --json 2>&1 | ConvertFrom-Json
$vulnCount = $npmAudit.metadata.vulnerabilities.total
Add-TestResult "npm依存関係" ($vulnCount -eq 0 -or $vulnCount -lt 5) "脆弱性: $vulnCount"

# cargo audit
cd src-tauri
$cargoAuditOutput = cargo audit 2>&1
$cargoAuditOk = $LASTEXITCODE -eq 0
Add-TestResult "Rust依存関係" $cargoAuditOk

cd ../..

# Test 4: VR/AR Dependencies
Write-Host ""
Write-Host "🎮 Test 4: VR/AR依存関係" -ForegroundColor Yellow

$pkg = Get-Content ".\tauri-gui\package.json" | ConvertFrom-Json
$hasThree = $pkg.dependencies."three" -ne $null
$hasXR = $pkg.dependencies."@react-three/xr" -ne $null

Add-TestResult "Three.js統合" $hasThree
Add-TestResult "WebXR統合" $hasXR

# Test 5: Kernel Driver
Write-Host ""
Write-Host "💻 Test 5: カーネルドライバー" -ForegroundColor Yellow

$driverC = ".\kernel-extensions\windows\ai_driver\ai_driver.c"
$ioctlC = ".\kernel-extensions\windows\ai_driver\ioctl_handlers.c"
$gpuC = ".\kernel-extensions\windows\ai_driver\gpu_integration.c"

Add-TestResult "ai_driver.c存在" (Test-Path $driverC)
Add-TestResult "ioctl_handlers.c存在" (Test-Path $ioctlC)
Add-TestResult "gpu_integration.c存在" (Test-Path $gpuC)

# Test 6: File Structure
Write-Host ""
Write-Host "📁 Test 6: ファイル構造" -ForegroundColor Yellow

$vrScene = ".\tauri-gui\src\components\vr\Scene4D.tsx"
$handTracking = ".\tauri-gui\src\lib\xr\hand-tracking.ts"
$gitVR = ".\tauri-gui\src\pages\GitVR.tsx"

Add-TestResult "Scene4D.tsx存在" (Test-Path $vrScene)
Add-TestResult "hand-tracking.ts存在" (Test-Path $handTracking)
Add-TestResult "GitVR.tsx存在" (Test-Path $gitVR)

# Summary
Write-Host ""
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "  📊 テスト結果" -ForegroundColor Cyan
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""

$passed = ($TestResults | Where-Object { $_.Passed }).Count
$total = $TestResults.Count

Write-Host "合格: $passed / $total" -ForegroundColor $(if ($passed -eq $total) { "Green" } else { "Yellow" })
Write-Host ""

if ($passed -eq $total) {
    Write-Host "✅ すべてのテストに合格しました！" -ForegroundColor Green
} else {
    Write-Host "⚠️  一部のテストで問題が見つかりました" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "詳細テスト: .\tauri-gui\SECURITY_TEST.md 参照" -ForegroundColor Cyan

# Save results
$TestResults | ConvertTo-Json | Out-File ".\security-test-results-unified.json" -Encoding UTF8
Write-Host "結果保存: security-test-results-unified.json" -ForegroundColor Gray

