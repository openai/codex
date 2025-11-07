# Codex AI Driver - 自動インストールスクリプト
# Windows 10/11用

<#
.SYNOPSIS
    Codex AI Driverを自動的にインストールします

.DESCRIPTION
    このスクリプトは以下を実行します：
    1. 管理者権限チェック
    2. システム要件確認
    3. テスト署名の有効化
    4. ドライバーのビルド（オプション）
    5. ドライバーのインストール
    6. 動作確認

.PARAMETER Build
    ドライバーをビルドするかどうか（デフォルト: false）

.PARAMETER Force
    確認なしで実行（デフォルト: false）

.EXAMPLE
    .\install-driver.ps1
    プリビルド版をインストール

.EXAMPLE
    .\install-driver.ps1 -Build
    ソースからビルドしてインストール

.EXAMPLE
    .\install-driver.ps1 -Force
    確認なしでインストール
#>

param(
    [switch]$Build = $false,
    [switch]$Force = $false
)

# エラー時に停止
$ErrorActionPreference = "Stop"

# 管理者権限チェック
function Test-Administrator {
    $currentUser = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $currentUser.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# プログレスバー表示
function Show-Progress {
    param(
        [string]$Activity,
        [string]$Status,
        [int]$PercentComplete
    )
    Write-Progress -Activity $Activity -Status $Status -PercentComplete $PercentComplete
}

# カラー出力
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

╔═══════════════════════════════════════════════════════╗
║                                                       ║
║        Codex AI Driver Installer v0.2.0              ║
║        Windows Kernel-Mode Driver                     ║
║                                                       ║
╚═══════════════════════════════════════════════════════╝

"@ -ForegroundColor Cyan
}

# システム要件チェック
function Test-SystemRequirements {
    Write-ColorOutput "`n[1/7] システム要件確認中..." -ForegroundColor Yellow
    Show-Progress -Activity "インストール準備" -Status "システム要件確認" -PercentComplete 10
    
    # Windows バージョンチェック
    $osVersion = [System.Environment]::OSVersion.Version
    if ($osVersion.Major -lt 10) {
        throw "このドライバーはWindows 10以降が必要です（現在: Windows $($osVersion.Major).$($osVersion.Minor)）"
    }
    Write-ColorOutput "  ✓ OS: Windows $($osVersion.Major).$($osVersion.Minor) Build $($osVersion.Build)" -ForegroundColor Green
    
    # アーキテクチャチェック
    if ([Environment]::Is64BitOperatingSystem -eq $false) {
        throw "64-bit Windowsが必要です"
    }
    Write-ColorOutput "  ✓ アーキテクチャ: 64-bit" -ForegroundColor Green
    
    # メモリチェック
    $totalMemory = (Get-CimInstance Win32_PhysicalMemory | Measure-Object -Property Capacity -Sum).Sum / 1GB
    if ($totalMemory -lt 8) {
        Write-ColorOutput "  ⚠ メモリ: $([math]::Round($totalMemory, 2))GB（推奨: 8GB以上）" -ForegroundColor Yellow
    } else {
        Write-ColorOutput "  ✓ メモリ: $([math]::Round($totalMemory, 2))GB" -ForegroundColor Green
    }
    
    # GPU チェック
    $gpu = Get-CimInstance Win32_VideoController | Select-Object -First 1
    Write-ColorOutput "  ✓ GPU: $($gpu.Name)" -ForegroundColor Green
}

# テスト署名確認
function Test-TestSigning {
    Write-ColorOutput "`n[2/7] テスト署名状態確認中..." -ForegroundColor Yellow
    Show-Progress -Activity "インストール準備" -Status "テスト署名確認" -PercentComplete 20
    
    $bcdeditOutput = bcdedit /enum | Select-String "testsigning"
    if ($bcdeditOutput -match "testsigning\s+Yes") {
        Write-ColorOutput "  ✓ テスト署名: 有効" -ForegroundColor Green
        return $true
    } else {
        Write-ColorOutput "  ✗ テスト署名: 無効" -ForegroundColor Red
        return $false
    }
}

# テスト署名有効化
function Enable-TestSigning {
    Write-ColorOutput "`n[3/7] テスト署名を有効化中..." -ForegroundColor Yellow
    Show-Progress -Activity "システム設定" -Status "テスト署名有効化" -PercentComplete 30
    
    if (-not $Force) {
        $response = Read-Host "テスト署名を有効化しますか？（再起動が必要です） [Y/n]"
        if ($response -ne "" -and $response -ne "Y" -and $response -ne "y") {
            throw "テスト署名の有効化がキャンセルされました"
        }
    }
    
    try {
        bcdedit /set testsigning on | Out-Null
        Write-ColorOutput "  ✓ テスト署名を有効化しました" -ForegroundColor Green
        Write-ColorOutput "  ⚠ 再起動後にこのスクリプトを再実行してください" -ForegroundColor Yellow
        
        if (-not $Force) {
            $response = Read-Host "今すぐ再起動しますか？ [Y/n]"
            if ($response -eq "" -or $response -eq "Y" -or $response -eq "y") {
                Restart-Computer
            }
        }
        exit 0
    } catch {
        throw "テスト署名の有効化に失敗しました: $_"
    }
}

# ドライバービルド
function Build-Driver {
    Write-ColorOutput "`n[4/7] ドライバービルド中..." -ForegroundColor Yellow
    Show-Progress -Activity "ドライバー準備" -Status "ビルド中" -PercentComplete 40
    
    # Visual Studio環境チェック
    $vsPath = "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\Launch-VsDevShell.ps1"
    if (-not (Test-Path $vsPath)) {
        $vsPath = "C:\Program Files (x86)\Microsoft Visual Studio\2019\Community\Common7\Tools\Launch-VsDevShell.ps1"
        if (-not (Test-Path $vsPath)) {
            throw "Visual Studio 2019/2022が見つかりません"
        }
    }
    
    # ビルド実行
    try {
        Push-Location "$PSScriptRoot\ai_driver"
        
        # VsDevShellを読み込み
        & $vsPath -SkipAutomaticLocation
        
        # MSBuildでビルド
        msbuild ai_driver.vcxproj /p:Configuration=Release /p:Platform=x64 /v:minimal
        
        if ($LASTEXITCODE -ne 0) {
            throw "ビルドが失敗しました（終了コード: $LASTEXITCODE）"
        }
        
        Write-ColorOutput "  ✓ ビルド完了" -ForegroundColor Green
        
        Pop-Location
    } catch {
        Pop-Location
        throw "ビルドエラー: $_"
    }
}

# 自己署名証明書作成
function New-TestCertificate {
    Write-ColorOutput "`n[5/7] テスト証明書作成中..." -ForegroundColor Yellow
    Show-Progress -Activity "ドライバー署名" -Status "証明書作成" -PercentComplete 50
    
    # 既存証明書確認
    $existingCert = Get-ChildItem Cert:\CurrentUser\My | 
        Where-Object { $_.Subject -eq "CN=Codex AI Driver Test Certificate" } | 
        Select-Object -First 1
    
    if ($existingCert) {
        Write-ColorOutput "  ✓ 既存の証明書を使用" -ForegroundColor Green
        return $existingCert
    }
    
    # 新規証明書作成
    $cert = New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject "CN=Codex AI Driver Test Certificate" `
        -KeyUsage DigitalSignature `
        -FriendlyName "Codex AI Driver Test" `
        -CertStoreLocation "Cert:\CurrentUser\My" `
        -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
    
    # 証明書エクスポート
    $certPath = Join-Path $PSScriptRoot "codex_test.cer"
    Export-Certificate -Cert $cert -FilePath $certPath | Out-Null
    
    # ストアに追加
    Import-Certificate -FilePath $certPath -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
    Import-Certificate -FilePath $certPath -CertStoreLocation Cert:\LocalMachine\TrustedPublisher | Out-Null
    
    Write-ColorOutput "  ✓ テスト証明書を作成しました" -ForegroundColor Green
    
    return $cert
}

# ドライバー署名
function Sign-Driver {
    param(
        [System.Security.Cryptography.X509Certificates.X509Certificate2]$Certificate
    )
    
    Write-ColorOutput "`n[6/7] ドライバー署名中..." -ForegroundColor Yellow
    Show-Progress -Activity "ドライバー署名" -Status "署名適用" -PercentComplete 60
    
    $driverPath = Join-Path $PSScriptRoot "ai_driver\ai_driver.sys"
    if (-not (Test-Path $driverPath)) {
        throw "ドライバーファイルが見つかりません: $driverPath"
    }
    
    # signtoolを探す
    $signtoolPaths = @(
        "C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe",
        "C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x64\signtool.exe"
    )
    
    $signtool = $signtoolPaths | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $signtool) {
        throw "signtool.exeが見つかりません（Windows SDKをインストールしてください）"
    }
    
    # 署名実行
    & $signtool sign /v /s My /n "Codex AI Driver Test Certificate" /t http://timestamp.digicert.com $driverPath
    
    if ($LASTEXITCODE -ne 0) {
        throw "署名が失敗しました（終了コード: $LASTEXITCODE）"
    }
    
    # 署名確認
    $signature = Get-AuthenticodeSignature $driverPath
    if ($signature.Status -ne "Valid") {
        Write-ColorOutput "  ⚠ 署名: $($signature.Status)" -ForegroundColor Yellow
    } else {
        Write-ColorOutput "  ✓ ドライバーに署名しました" -ForegroundColor Green
    }
}

# ドライバーインストール
function Install-Driver {
    Write-ColorOutput "`n[7/7] ドライバーインストール中..." -ForegroundColor Yellow
    Show-Progress -Activity "ドライバーインストール" -Status "インストール実行" -PercentComplete 80
    
    $infPath = Join-Path $PSScriptRoot "ai_driver\ai_driver.inf"
    if (-not (Test-Path $infPath)) {
        throw "INFファイルが見つかりません: $infPath"
    }
    
    try {
        # pnputilでインストール
        pnputil /add-driver $infPath /install
        
        if ($LASTEXITCODE -eq 0) {
            Write-ColorOutput "  ✓ ドライバーをインストールしました" -ForegroundColor Green
        } else {
            throw "pnputilが失敗しました（終了コード: $LASTEXITCODE）"
        }
        
        # サービス開始
        Start-Sleep -Seconds 2
        sc.exe start AI_Driver | Out-Null
        
        if ($LASTEXITCODE -eq 0 -or $LASTEXITCODE -eq 1056) {
            # 1056 = サービスは既に実行中
            Write-ColorOutput "  ✓ ドライバーサービスを開始しました" -ForegroundColor Green
        } else {
            Write-ColorOutput "  ⚠ サービス開始に失敗しました（手動で起動してください）" -ForegroundColor Yellow
        }
        
    } catch {
        throw "インストールエラー: $_"
    }
}

# 動作確認
function Test-DriverInstallation {
    Write-ColorOutput "`n[✓] 動作確認中..." -ForegroundColor Yellow
    Show-Progress -Activity "インストール完了" -Status "動作確認" -PercentComplete 90
    
    # サービス状態確認
    $service = Get-Service -Name "AI_Driver" -ErrorAction SilentlyContinue
    if ($service) {
        Write-ColorOutput "  ✓ サービス: $($service.Status)" -ForegroundColor Green
    } else {
        Write-ColorOutput "  ✗ サービスが見つかりません" -ForegroundColor Red
        return $false
    }
    
    # モジュール確認
    $driver = Get-WindowsDriver -Online | Where-Object { $_.OriginalFileName -like "*ai_driver*" }
    if ($driver) {
        Write-ColorOutput "  ✓ ドライバーバージョン: $($driver.Version)" -ForegroundColor Green
    }
    
    return $true
}

# インストール完了メッセージ
function Show-CompletionMessage {
    Show-Progress -Activity "インストール完了" -Status "完了" -PercentComplete 100
    
    Write-ColorOutput @"

╔═══════════════════════════════════════════════════════╗
║                                                       ║
║        ✓ インストール完了！                          ║
║                                                       ║
╚═══════════════════════════════════════════════════════╝

次のステップ:

1. Codex統合ツールで確認:
   cd codex_win_api
   cargo run --release

2. サービス状態確認:
   sc query AI_Driver

3. ベンチマーク実行:
   cd ..\benchmarks
   py -3 stress_test.py --with-driver

詳細: INSTALL.md を参照

"@ -ForegroundColor Green
}

# メイン処理
function Main {
    try {
        Show-Banner
        
        # 管理者権限チェック
        if (-not (Test-Administrator)) {
            throw "このスクリプトは管理者権限で実行する必要があります"
        }
        
        # システム要件確認
        Test-SystemRequirements
        
        # テスト署名確認
        if (-not (Test-TestSigning)) {
            Enable-TestSigning
            return
        }
        
        # ビルド（オプション）
        if ($Build) {
            Build-Driver
        }
        
        # 証明書作成
        $cert = New-TestCertificate
        
        # 署名
        Sign-Driver -Certificate $cert
        
        # インストール
        Install-Driver
        
        # 動作確認
        $success = Test-DriverInstallation
        
        if ($success) {
            Show-CompletionMessage
            
            # 完了音再生
            $soundPath = "C:\Users\downl\Desktop\SO8T\.cursor\marisa_owattaze.wav"
            if (Test-Path $soundPath) {
                $player = New-Object System.Media.SoundPlayer $soundPath
                $player.PlaySync()
            }
        } else {
            Write-ColorOutput "`n⚠ インストールは完了しましたが、ドライバーが正常に動作していません" -ForegroundColor Yellow
            Write-ColorOutput "トラブルシューティング: INSTALL.md を参照" -ForegroundColor Yellow
        }
        
    } catch {
        Write-ColorOutput "`n✗ エラー: $_" -ForegroundColor Red
        Write-ColorOutput "`n詳細ログ:" -ForegroundColor Yellow
        Write-ColorOutput $_.Exception.Message -ForegroundColor Red
        exit 1
    }
}

# スクリプト実行
Main




