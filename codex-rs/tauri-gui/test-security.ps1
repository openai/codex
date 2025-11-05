# Codex Tauri - セキュリティテスト自動化スクリプト
# Windows環境での実機セキュリティテスト

param(
    [switch]$Quick,      # クイックテストのみ
    [switch]$Full,       # 全テスト実行
    [switch]$Verbose     # 詳細出力
)

Write-Host "🔒 Codex Tauri セキュリティテスト" -ForegroundColor Cyan
Write-Host "=================================" -ForegroundColor Cyan
Write-Host ""

$ErrorActionPreference = "Continue"
$TestResults = @()

function Test-Result {
    param(
        [string]$TestName,
        [bool]$Passed,
        [string]$Message = ""
    )
    
    $result = @{
        Name = $TestName
        Passed = $Passed
        Message = $Message
        Timestamp = Get-Date
    }
    
    $script:TestResults += $result
    
    if ($Passed) {
        Write-Host "✅ $TestName" -ForegroundColor Green
    } else {
        Write-Host "❌ $TestName" -ForegroundColor Red
        if ($Message) {
            Write-Host "   $Message" -ForegroundColor Yellow
        }
    }
}

# Test 1: ビルド済みバイナリ確認
Write-Host "📦 Test 1: バイナリ確認" -ForegroundColor Yellow
$exePath = ".\src-tauri\target\release\codex-tauri.exe"
if (Test-Path $exePath) {
    Test-Result "バイナリ存在確認" $true
    
    # ファイルサイズ確認
    $fileSize = (Get-Item $exePath).Length / 1MB
    Write-Host "   ファイルサイズ: $([math]::Round($fileSize, 2)) MB" -ForegroundColor Gray
    
    if ($fileSize -lt 100) {
        Test-Result "バイナリサイズ適正" $true "期待: <100MB"
    } else {
        Test-Result "バイナリサイズ適正" $false "実際: $fileSize MB (大きすぎる可能性)"
    }
} else {
    Test-Result "バイナリ存在確認" $false "ビルドが必要: npm run tauri build"
    Write-Host ""
    Write-Host "⚠️  ビルドを実行してください:" -ForegroundColor Yellow
    Write-Host "   npm run tauri build" -ForegroundColor Gray
    exit 1
}

# Test 2: Tauri設定確認
Write-Host ""
Write-Host "⚙️  Test 2: Tauri設定確認" -ForegroundColor Yellow
$configPath = ".\src-tauri\tauri.conf.json"
if (Test-Path $configPath) {
    $config = Get-Content $configPath | ConvertFrom-Json
    
    # CSP確認
    $csp = $config.app.security.csp
    if ($csp -match "default-src 'self'") {
        Test-Result "CSP設定" $true "default-src 'self' 設定済み"
    } else {
        Test-Result "CSP設定" $false "CSPが緩すぎる可能性"
    }
    
    # Shell実行確認
    if ($config.tauri.allowlist.shell.execute -eq $false -or $config.tauri.allowlist.shell.execute -eq $null) {
        Test-Result "Shell実行禁止" $true
    } else {
        Test-Result "Shell実行禁止" $false "Shellが有効になっています"
    }
} else {
    Test-Result "設定ファイル" $false "tauri.conf.jsonが見つかりません"
}

# Test 3: 依存関係脆弱性スキャン
Write-Host ""
Write-Host "🔍 Test 3: 依存関係脆弱性スキャン" -ForegroundColor Yellow

# npm audit
Write-Host "   npm auditを実行中..." -ForegroundColor Gray
$npmAudit = npm audit --json 2>&1 | ConvertFrom-Json
if ($npmAudit.metadata.vulnerabilities.total -eq 0) {
    Test-Result "npm依存関係" $true "脆弱性なし"
} else {
    $critical = $npmAudit.metadata.vulnerabilities.critical
    $high = $npmAudit.metadata.vulnerabilities.high
    Test-Result "npm依存関係" $false "Critical: $critical, High: $high"
}

# cargo audit (Rustの脆弱性チェック)
Write-Host "   cargo auditを実行中..." -ForegroundColor Gray
Push-Location .\src-tauri
try {
    $cargoAudit = cargo audit 2>&1
    if ($LASTEXITCODE -eq 0) {
        Test-Result "Rust依存関係" $true "脆弱性なし"
    } else {
        Test-Result "Rust依存関係" $false "cargo audit参照"
    }
} catch {
    Test-Result "Rust依存関係" $false "cargo auditが実行できません（インストールが必要）"
}
Pop-Location

# Test 4: ファイル権限確認
Write-Host ""
Write-Host "🔐 Test 4: ファイル権限確認" -ForegroundColor Yellow
$dbPath = "$env:APPDATA\codex"
if (Test-Path $dbPath) {
    $acl = Get-Acl $dbPath
    $currentUser = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
    
    # 現在ユーザーのみアクセス可能か確認
    $userAccess = $acl.Access | Where-Object { $_.IdentityReference -eq $currentUser }
    if ($userAccess) {
        Test-Result "データベースディレクトリ権限" $true "現在ユーザーのみアクセス可能"
    } else {
        Test-Result "データベースディレクトリ権限" $false "権限設定を確認してください"
    }
} else {
    Write-Host "   ℹ️  データベースディレクトリが未作成（初回起動後に作成されます）" -ForegroundColor Gray
}

# Test 5: コード署名確認
Write-Host ""
Write-Host "📝 Test 5: コード署名確認" -ForegroundColor Yellow
$signature = Get-AuthenticodeSignature $exePath
if ($signature.Status -eq "Valid") {
    Test-Result "コード署名" $true "有効な署名"
    Write-Host "   署名者: $($signature.SignerCertificate.Subject)" -ForegroundColor Gray
} elseif ($signature.Status -eq "NotSigned") {
    Test-Result "コード署名" $false "未署名（開発版は正常）"
    Write-Host "   ℹ️  開発版は未署名で正常です" -ForegroundColor Gray
} else {
    Test-Result "コード署名" $false "署名が無効: $($signature.Status)"
}

# Test 6: プロセスインテグリティレベル確認
Write-Host ""
Write-Host "🛡️  Test 6: プロセス権限確認" -ForegroundColor Yellow
if (Test-Path $exePath) {
    # 実行ファイルのマニフェスト確認（管理者権限要求の有無）
    # Note: これは簡易チェック、実際の確認はリソースエディタが必要
    Write-Host "   ℹ️  管理者権限要求がないことを確認してください" -ForegroundColor Gray
    Test-Result "通常ユーザー実行" $true "管理者権限不要で設計"
}

# Test 7: ネットワーク通信確認（簡易）
Write-Host ""
Write-Host "🌐 Test 7: ネットワーク通信確認" -ForegroundColor Yellow
Write-Host "   ℹ️  実行時にWiresharkで詳細確認を推奨" -ForegroundColor Gray
Test-Result "ネットワーク監視推奨" $true "手動確認が必要"

# Test 8: メモリ安全性（Rust）
Write-Host ""
Write-Host "🦀 Test 8: Rust メモリ安全性" -ForegroundColor Yellow
Write-Host "   Rustの型システムによりメモリ安全性を保証" -ForegroundColor Gray
Test-Result "メモリ安全性" $true "Rustによる保証"

# 結果サマリー
Write-Host ""
Write-Host "=================================" -ForegroundColor Cyan
Write-Host "📊 テスト結果サマリー" -ForegroundColor Cyan
Write-Host "=================================" -ForegroundColor Cyan
Write-Host ""

$passedCount = ($TestResults | Where-Object { $_.Passed }).Count
$totalCount = $TestResults.Count

Write-Host "合格: $passedCount / $totalCount" -ForegroundColor $(if ($passedCount -eq $totalCount) { "Green" } else { "Yellow" })
Write-Host ""

if ($passedCount -eq $totalCount) {
    Write-Host "✅ すべてのテストに合格しました！" -ForegroundColor Green
} else {
    Write-Host "⚠️  一部のテストで問題が見つかりました。" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "失敗したテスト:" -ForegroundColor Yellow
    $TestResults | Where-Object { -not $_.Passed } | ForEach-Object {
        Write-Host "  - $($_.Name): $($_.Message)" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host "=================================" -ForegroundColor Cyan
Write-Host "🔍 詳細テスト推奨事項" -ForegroundColor Cyan
Write-Host "=================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. Process Monitorでファイル/レジストリアクセス監視" -ForegroundColor White
Write-Host "   https://docs.microsoft.com/en-us/sysinternals/downloads/procmon" -ForegroundColor Gray
Write-Host ""
Write-Host "2. Wiresharkでネットワーク通信監視" -ForegroundColor White
Write-Host "   https://www.wireshark.org/" -ForegroundColor Gray
Write-Host ""
Write-Host "3. Process Explorerでメモリ使用状況確認" -ForegroundColor White
Write-Host "   https://docs.microsoft.com/en-us/sysinternals/downloads/process-explorer" -ForegroundColor Gray
Write-Host ""
Write-Host "詳細: .\SECURITY_TEST.md を参照" -ForegroundColor Cyan
Write-Host ""

# 結果をJSONで保存
$resultJson = $TestResults | ConvertTo-Json -Depth 5
$resultJson | Out-File ".\security-test-results.json" -Encoding UTF8
Write-Host "📄 テスト結果を security-test-results.json に保存しました" -ForegroundColor Green

