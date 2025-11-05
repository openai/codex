# Codex AI Driver - アンインストールスクリプト
# Windows 10/11用

<#
.SYNOPSIS
    Codex AI Driverをアンインストールします

.DESCRIPTION
    ドライバーを完全に削除し、システムをクリーンな状態に戻します

.PARAMETER KeepTestSigning
    テスト署名モードを維持する（デフォルト: false）

.PARAMETER Force
    確認なしで実行（デフォルト: false）

.EXAMPLE
    .\uninstall-driver.ps1
    ドライバーをアンインストール

.EXAMPLE
    .\uninstall-driver.ps1 -KeepTestSigning
    テスト署名を維持したままアンインストール
#>

param(
    [switch]$KeepTestSigning = $false,
    [switch]$Force = $false
)

$ErrorActionPreference = "Stop"

# 管理者権限チェック
function Test-Administrator {
    $currentUser = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $currentUser.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
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
║        Codex AI Driver Uninstaller v0.2.0            ║
║                                                       ║
╚═══════════════════════════════════════════════════════╝

"@ -ForegroundColor Cyan
}

# サービス停止
function Stop-DriverService {
    Write-ColorOutput "[1/5] サービス停止中..." -ForegroundColor Yellow
    
    $service = Get-Service -Name "AI_Driver" -ErrorAction SilentlyContinue
    if ($service) {
        if ($service.Status -eq "Running") {
            Stop-Service -Name "AI_Driver" -Force
            Write-ColorOutput "  ✓ サービスを停止しました" -ForegroundColor Green
        } else {
            Write-ColorOutput "  ✓ サービスは既に停止しています" -ForegroundColor Green
        }
    } else {
        Write-ColorOutput "  ! サービスが見つかりません（既に削除済み）" -ForegroundColor Yellow
    }
}

# ドライバー削除
function Remove-Driver {
    Write-ColorOutput "`n[2/5] ドライバー削除中..." -ForegroundColor Yellow
    
    try {
        # INFファイルを検索
        $drivers = pnputil /enum-drivers | Select-String -Pattern "ai_driver\.inf"
        
        if ($drivers) {
            foreach ($line in $drivers) {
                if ($line -match "Published Name\s*:\s*(\S+)") {
                    $oemInf = $Matches[1]
                    Write-ColorOutput "  削除中: $oemInf" -ForegroundColor Gray
                    pnputil /delete-driver $oemInf /uninstall /force
                }
            }
            Write-ColorOutput "  ✓ ドライバーを削除しました" -ForegroundColor Green
        } else {
            Write-ColorOutput "  ! ドライバーが見つかりません（既に削除済み）" -ForegroundColor Yellow
        }
    } catch {
        Write-ColorOutput "  ⚠ ドライバー削除でエラーが発生しました: $_" -ForegroundColor Yellow
    }
}

# レジストリクリーンアップ
function Remove-RegistryEntries {
    Write-ColorOutput "`n[3/5] レジストリクリーンアップ中..." -ForegroundColor Yellow
    
    $regPaths = @(
        "HKLM:\SYSTEM\CurrentControlSet\Services\AI_Driver",
        "HKLM:\SYSTEM\ControlSet001\Services\AI_Driver",
        "HKLM:\SYSTEM\ControlSet002\Services\AI_Driver"
    )
    
    $removed = $false
    foreach ($regPath in $regPaths) {
        if (Test-Path $regPath) {
            Remove-Item $regPath -Recurse -Force
            Write-ColorOutput "  削除: $regPath" -ForegroundColor Gray
            $removed = $true
        }
    }
    
    if ($removed) {
        Write-ColorOutput "  ✓ レジストリエントリを削除しました" -ForegroundColor Green
    } else {
        Write-ColorOutput "  ! レジストリエントリが見つかりません（既に削除済み）" -ForegroundColor Yellow
    }
}

# 証明書削除
function Remove-Certificates {
    Write-ColorOutput "`n[4/5] テスト証明書削除中..." -ForegroundColor Yellow
    
    $stores = @(
        "Cert:\CurrentUser\My",
        "Cert:\LocalMachine\Root",
        "Cert:\LocalMachine\TrustedPublisher"
    )
    
    $removed = $false
    foreach ($store in $stores) {
        $certs = Get-ChildItem $store -ErrorAction SilentlyContinue | 
            Where-Object { $_.Subject -eq "CN=Codex AI Driver Test Certificate" }
        
        foreach ($cert in $certs) {
            Remove-Item $cert.PSPath -Force
            Write-ColorOutput "  削除: $store" -ForegroundColor Gray
            $removed = $true
        }
    }
    
    # 証明書ファイル削除
    $certFile = Join-Path $PSScriptRoot "codex_test.cer"
    if (Test-Path $certFile) {
        Remove-Item $certFile -Force
        $removed = $true
    }
    
    if ($removed) {
        Write-ColorOutput "  ✓ 証明書を削除しました" -ForegroundColor Green
    } else {
        Write-ColorOutput "  ! 証明書が見つかりません（既に削除済み）" -ForegroundColor Yellow
    }
}

# テスト署名無効化
function Disable-TestSigning {
    Write-ColorOutput "`n[5/5] テスト署名無効化中..." -ForegroundColor Yellow
    
    if ($KeepTestSigning) {
        Write-ColorOutput "  ! テスト署名は維持します（-KeepTestSigningが指定されました）" -ForegroundColor Yellow
        return
    }
    
    $bcdeditOutput = bcdedit /enum | Select-String "testsigning"
    if ($bcdeditOutput -match "testsigning\s+Yes") {
        if (-not $Force) {
            $response = Read-Host "テスト署名を無効化しますか？（再起動が必要です） [Y/n]"
            if ($response -ne "" -and $response -ne "Y" -and $response -ne "y") {
                Write-ColorOutput "  ! テスト署名は維持します" -ForegroundColor Yellow
                return
            }
        }
        
        bcdedit /set testsigning off | Out-Null
        Write-ColorOutput "  ✓ テスト署名を無効化しました" -ForegroundColor Green
        Write-ColorOutput "  ⚠ 変更を適用するには再起動が必要です" -ForegroundColor Yellow
    } else {
        Write-ColorOutput "  ! テスト署名は既に無効です" -ForegroundColor Yellow
    }
}

# アンインストール確認
function Confirm-Uninstallation {
    Write-ColorOutput "`n[確認] アンインストール検証中..." -ForegroundColor Yellow
    
    $issues = @()
    
    # サービス確認
    $service = Get-Service -Name "AI_Driver" -ErrorAction SilentlyContinue
    if ($service) {
        $issues += "サービスがまだ存在します"
    }
    
    # ドライバー確認
    $driver = Get-WindowsDriver -Online | Where-Object { $_.OriginalFileName -like "*ai_driver*" }
    if ($driver) {
        $issues += "ドライバーがまだ登録されています"
    }
    
    # レジストリ確認
    if (Test-Path "HKLM:\SYSTEM\CurrentControlSet\Services\AI_Driver") {
        $issues += "レジストリエントリが残っています"
    }
    
    if ($issues.Count -eq 0) {
        Write-ColorOutput "  ✓ アンインストール完了" -ForegroundColor Green
        return $true
    } else {
        Write-ColorOutput "  ⚠ 以下の問題が見つかりました:" -ForegroundColor Yellow
        foreach ($issue in $issues) {
            Write-ColorOutput "    - $issue" -ForegroundColor Yellow
        }
        Write-ColorOutput "  ⚠ 再起動後に再度確認してください" -ForegroundColor Yellow
        return $false
    }
}

# 完了メッセージ
function Show-CompletionMessage {
    param([bool]$NeedsReboot)
    
    Write-ColorOutput @"

╔═══════════════════════════════════════════════════════╗
║                                                       ║
║        ✓ アンインストール完了                        ║
║                                                       ║
╚═══════════════════════════════════════════════════════╝

"@ -ForegroundColor Green
    
    if ($NeedsReboot) {
        Write-ColorOutput "⚠ 変更を完全に適用するには再起動が必要です`n" -ForegroundColor Yellow
        
        if (-not $Force) {
            $response = Read-Host "今すぐ再起動しますか？ [Y/n]"
            if ($response -eq "" -or $response -eq "Y" -or $response -eq "y") {
                Restart-Computer
            }
        }
    }
}

# メイン処理
function Main {
    try {
        Show-Banner
        
        # 管理者権限チェック
        if (-not (Test-Administrator)) {
            throw "このスクリプトは管理者権限で実行する必要があります"
        }
        
        # 確認
        if (-not $Force) {
            Write-ColorOutput "Codex AI Driverをアンインストールします。" -ForegroundColor Yellow
            $response = Read-Host "続行しますか？ [Y/n]"
            if ($response -ne "" -and $response -ne "Y" -and $response -ne "y") {
                Write-ColorOutput "アンインストールをキャンセルしました" -ForegroundColor Yellow
                exit 0
            }
        }
        
        # アンインストール実行
        Stop-DriverService
        Remove-Driver
        Remove-RegistryEntries
        Remove-Certificates
        Disable-TestSigning
        
        # 検証
        $success = Confirm-Uninstallation
        
        # 完了
        $needsReboot = (-not $KeepTestSigning) -and (bcdedit /enum | Select-String "testsigning" | Select-String "No")
        Show-CompletionMessage -NeedsReboot $needsReboot
        
    } catch {
        Write-ColorOutput "`n✗ エラー: $_" -ForegroundColor Red
        Write-ColorOutput $_.Exception.Message -ForegroundColor Red
        exit 1
    }
}

# スクリプト実行
Main

