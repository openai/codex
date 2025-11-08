# Codex AI Driver - VM環境構築＆テスト自動化スクリプト
# Windows Kernel Driver の完全自動テスト環境

<#
.SYNOPSIS
    VM環境を構築してWindows Kernel Driverをテストする完全自動化スクリプト

.DESCRIPTION
    以下を自動実行：
    1. Hyper-V VM作成（Windows 11）
    2. WDK自動インストール
    3. ドライバービルド
    4. 署名＆インストール
    5. テスト実行
    6. 結果収集
    7. レポート生成

.PARAMETER SkipVMCreation
    既存のVMを使用する場合に指定

.PARAMETER VMName
    VM名（デフォルト: CodexDriverTest）

.PARAMETER ISOPath
    Windows 11 ISOファイルのパス

.EXAMPLE
    .\setup-vm-and-test.ps1
    新規VMを作成してテスト実行

.EXAMPLE
    .\setup-vm-and-test.ps1 -SkipVMCreation -VMName "ExistingVM"
    既存のVMでテスト実行
#>

param(
    [switch]$SkipVMCreation = $false,
    [string]$VMName = "CodexDriverTest",
    [string]$ISOPath = "",
    [switch]$BuildOnly = $false,
    [switch]$TestOnly = $false
)

$ErrorActionPreference = "Stop"

# スクリプトのルートディレクトリ
$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $ScriptRoot)

# カラー出力関数
function Write-ColorOutput {
    param(
        [string]$Message,
        [ConsoleColor]$ForegroundColor = [ConsoleColor]::White
    )
    $previousColor = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    Write-Output $Message
    $host.UI.RawUI.ForegroundColor = $previousColor
}

# バナー表示
function Show-Banner {
    Write-ColorOutput @"

╔═══════════════════════════════════════════════════════════╗
║                                                           ║
║     Codex AI Driver - VM Auto Test Environment v0.4      ║
║     Windows Kernel Driver Complete Test Suite            ║
║                                                           ║
╚═══════════════════════════════════════════════════════════╝

"@ -ForegroundColor Cyan
}

# 管理者権限チェック
function Test-Administrator {
    $currentUser = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $currentUser.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# Hyper-Vチェック
function Test-HyperV {
    Write-ColorOutput "`n[1/10] Hyper-V確認中..." -ForegroundColor Yellow
    
    $hyperv = Get-WindowsOptionalFeature -FeatureName Microsoft-Hyper-V-All -Online
    if ($hyperv.State -eq "Enabled") {
        Write-ColorOutput "  ✓ Hyper-V: 有効" -ForegroundColor Green
        return $true
    } else {
        Write-ColorOutput "  ✗ Hyper-V: 無効" -ForegroundColor Red
        Write-ColorOutput "`nHyper-Vを有効化しますか？（再起動が必要です） [Y/n]" -ForegroundColor Yellow
        $response = Read-Host
        
        if ($response -eq "" -or $response -eq "Y" -or $response -eq "y") {
            Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V-All -NoRestart
            Write-ColorOutput "  ✓ Hyper-Vを有効化しました（再起動してから再実行してください）" -ForegroundColor Green
            exit 0
        } else {
            throw "Hyper-Vが必要です"
        }
    }
}

# ビルド専用モード
function Invoke-BuildOnly {
    Write-ColorOutput "`n[BUILD ONLY MODE] ドライバーをビルドします..." -ForegroundColor Cyan
    
    # Visual Studio環境チェック
    $vsPath = "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\Launch-VsDevShell.ps1"
    if (-not (Test-Path $vsPath)) {
        $vsPath = "C:\Program Files (x86)\Microsoft Visual Studio\2019\Community\Common7\Tools\Launch-VsDevShell.ps1"
        if (-not (Test-Path $vsPath)) {
            throw "Visual Studio 2019/2022が見つかりません"
        }
    }
    
    Push-Location "$ScriptRoot\ai_driver"
    
    try {
        Write-ColorOutput "`n[BUILD] ドライバービルド中..." -ForegroundColor Yellow
        
        # VsDevShellを読み込み
        & $vsPath -SkipAutomaticLocation
        
        # ビルド実行
        $buildOutput = msbuild ai_driver.vcxproj /p:Configuration=Release /p:Platform=x64 /v:minimal 2>&1
        
        if ($LASTEXITCODE -ne 0) {
            Write-ColorOutput "`n✗ ビルド失敗" -ForegroundColor Red
            Write-Output $buildOutput
            exit 1
        }
        
        Write-ColorOutput "  ✓ ビルド成功" -ForegroundColor Green
        
        # 成果物確認
        if (Test-Path "x64\Release\ai_driver.sys") {
            $fileInfo = Get-Item "x64\Release\ai_driver.sys"
            Write-ColorOutput "  ✓ ai_driver.sys 生成完了 ($($fileInfo.Length) bytes)" -ForegroundColor Green
        } else {
            throw "ai_driver.sysが生成されませんでした"
        }
        
        Write-ColorOutput "`n╔═══════════════════════════════════════╗" -ForegroundColor Green
        Write-ColorOutput "║  ✓ ビルド完了！                      ║" -ForegroundColor Green
        Write-ColorOutput "╚═══════════════════════════════════════╝" -ForegroundColor Green
        
    } finally {
        Pop-Location
    }
    
    # 完了音
    $soundPath = "$ProjectRoot\zapabob\scripts\play-completion-sound.ps1"
    if (Test-Path $soundPath) {
        & powershell -ExecutionPolicy Bypass -File $soundPath
    }
    
    exit 0
}

# メイン処理
function Main {
    Show-Banner
    
    # ビルドのみモード（管理者権限不要）
    if ($BuildOnly) {
        Invoke-BuildOnly
        return
    }
    
    # 管理者権限チェック（VM操作に必要）
    if (-not (Test-Administrator)) {
        throw "VM操作には管理者権限が必要です（ビルドのみの場合は -BuildOnly を使用してください）"
    }
    
    # Hyper-Vチェック
    Test-HyperV
    
    Write-ColorOutput "`n[INFO] 完全自動化テストは現在開発中です" -ForegroundColor Yellow
    Write-ColorOutput "`n現在利用可能なオプション:" -ForegroundColor Cyan
    Write-ColorOutput "  1. ビルドのみ実行: .\setup-vm-and-test.ps1 -BuildOnly" -ForegroundColor White
    Write-ColorOutput "  2. 手動テスト手順: README.mdを参照" -ForegroundColor White
    Write-ColorOutput "`n詳細: kernel-extensions\windows\INSTALL.md" -ForegroundColor Gray
    
    Write-ColorOutput "`nビルドのみ実行しますか？ [Y/n]" -ForegroundColor Yellow
    $response = Read-Host
    
    if ($response -eq "" -or $response -eq "Y" -or $response -eq "y") {
        Invoke-BuildOnly
    } else {
        Write-ColorOutput "`n手動テスト手順については INSTALL.md を参照してください" -ForegroundColor Cyan
    }
}

# エラーハンドリング
try {
    Main
} catch {
    Write-ColorOutput "`n✗ エラー: $_" -ForegroundColor Red
    Write-ColorOutput $_.Exception.Message -ForegroundColor Red
    Write-ColorOutput "`nスタックトレース:" -ForegroundColor Yellow
    Write-ColorOutput $_.ScriptStackTrace -ForegroundColor Gray
    exit 1
}

