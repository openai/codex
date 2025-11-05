# Codex AI Driver - 診断情報収集スクリプト

<#
.SYNOPSIS
    Codex AI Driverの診断情報を収集します

.DESCRIPTION
    トラブルシューティングに必要な情報を自動収集し、ZIPファイルに保存します
    
    収集内容:
    - システム情報
    - ドライバー状態
    - イベントログ
    - レジストリダンプ
    - GPU情報
    - サービス状態

.EXAMPLE
    .\collect-driver-diagnostics.ps1
    診断情報を収集して diagnostics-*.zip を作成
#>

$ErrorActionPreference = "Continue"

# 出力ディレクトリ作成
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$outputDir = Join-Path $PSScriptRoot "..\diagnostics-$timestamp"
New-Item -ItemType Directory -Path $outputDir -Force | Out-Null

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

Write-ColorOutput @"

╔═══════════════════════════════════════════════════════╗
║                                                       ║
║     Codex AI Driver Diagnostics Collector v0.2.0     ║
║                                                       ║
╚═══════════════════════════════════════════════════════╝

"@ -ForegroundColor Cyan

# 1. システム情報
Write-ColorOutput "[1/8] システム情報収集中..." -ForegroundColor Yellow
$systemInfo = @{
    OS = (Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber, OSArchitecture)
    CPU = (Get-CimInstance Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors)
    Memory = (Get-CimInstance Win32_PhysicalMemory | Measure-Object -Property Capacity -Sum | Select-Object @{N="TotalGB";E={$_.Sum / 1GB}})
    Uptime = (Get-CimInstance Win32_OperatingSystem).LastBootUpTime
}
$systemInfo | ConvertTo-Json -Depth 10 | Out-File (Join-Path $outputDir "system-info.json")
Write-ColorOutput "  ✓ system-info.json" -ForegroundColor Green

# 2. GPU情報
Write-ColorOutput "[2/8] GPU情報収集中..." -ForegroundColor Yellow
$gpuInfo = Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion, DriverDate, AdapterRAM
$gpuInfo | ConvertTo-Json -Depth 10 | Out-File (Join-Path $outputDir "gpu-info.json")
Write-ColorOutput "  ✓ gpu-info.json" -ForegroundColor Green

# 3. ドライバー状態
Write-ColorOutput "[3/8] ドライバー情報収集中..." -ForegroundColor Yellow
$driverInfo = @{}

# pnputil情報
$pnpOutput = pnputil /enum-drivers | Out-String
$pnpOutput | Out-File (Join-Path $outputDir "pnputil-drivers.txt")

# 特定ドライバー情報
$aiDriver = Get-WindowsDriver -Online | Where-Object { $_.OriginalFileName -like "*ai_driver*" }
if ($aiDriver) {
    $driverInfo["AI_Driver"] = $aiDriver | Select-Object *
}

$driverInfo | ConvertTo-Json -Depth 10 | Out-File (Join-Path $outputDir "driver-info.json")
Write-ColorOutput "  ✓ driver-info.json" -ForegroundColor Green

# 4. サービス状態
Write-ColorOutput "[4/8] サービス情報収集中..." -ForegroundColor Yellow
$service = Get-Service -Name "AI_Driver" -ErrorAction SilentlyContinue
if ($service) {
    $serviceInfo = $service | Select-Object *
    $serviceInfo | ConvertTo-Json -Depth 10 | Out-File (Join-Path $outputDir "service-info.json")
    Write-ColorOutput "  ✓ service-info.json (Status: $($service.Status))" -ForegroundColor Green
} else {
    "サービスが見つかりません" | Out-File (Join-Path $outputDir "service-info.txt")
    Write-ColorOutput "  ! サービスが見つかりません" -ForegroundColor Yellow
}

# 5. レジストリダンプ
Write-ColorOutput "[5/8] レジストリ情報収集中..." -ForegroundColor Yellow
$regPaths = @(
    "HKLM:\SYSTEM\CurrentControlSet\Services\AI_Driver",
    "HKLM:\SYSTEM\CurrentControlSet\Enum\Root\AI_Driver"
)

foreach ($regPath in $regPaths) {
    if (Test-Path $regPath) {
        $regName = $regPath -replace ":", "" -replace "\\", "-"
        $regOutput = reg export $regPath "$outputDir\registry-$regName.reg" 2>&1
        Write-ColorOutput "  ✓ registry-$regName.reg" -ForegroundColor Green
    }
}

# 6. イベントログ
Write-ColorOutput "[6/8] イベントログ収集中..." -ForegroundColor Yellow

# システムログ（直近100件）
$systemEvents = Get-EventLog -LogName System -Newest 100 -ErrorAction SilentlyContinue
if ($systemEvents) {
    $systemEvents | Select-Object TimeGenerated, EntryType, Source, Message | 
        Export-Csv (Join-Path $outputDir "eventlog-system.csv") -NoTypeInformation
    Write-ColorOutput "  ✓ eventlog-system.csv" -ForegroundColor Green
}

# ドライバー関連イベント
$driverEvents = Get-WinEvent -LogName "Microsoft-Windows-DriverFrameworks-UserMode/Operational" -MaxEvents 100 -ErrorAction SilentlyContinue |
    Where-Object { $_.Message -like "*AI_Driver*" }
if ($driverEvents) {
    $driverEvents | Select-Object TimeCreated, Level, Message | 
        Export-Csv (Join-Path $outputDir "eventlog-driver.csv") -NoTypeInformation
    Write-ColorOutput "  ✓ eventlog-driver.csv" -ForegroundColor Green
}

# 7. bcdedit情報
Write-ColorOutput "[7/8] ブート設定情報収集中..." -ForegroundColor Yellow
bcdedit /enum | Out-File (Join-Path $outputDir "bcdedit-enum.txt")
Write-ColorOutput "  ✓ bcdedit-enum.txt" -ForegroundColor Green

# 8. 証明書情報
Write-ColorOutput "[8/8] 証明書情報収集中..." -ForegroundColor Yellow
$certs = @()
$stores = @("Cert:\CurrentUser\My", "Cert:\LocalMachine\Root", "Cert:\LocalMachine\TrustedPublisher")
foreach ($store in $stores) {
    $storeCerts = Get-ChildItem $store -ErrorAction SilentlyContinue | 
        Where-Object { $_.Subject -like "*Codex*" -or $_.Subject -like "*AI Driver*" }
    $certs += $storeCerts | Select-Object PSPath, Subject, Issuer, NotBefore, NotAfter, Thumbprint
}
$certs | Export-Csv (Join-Path $outputDir "certificates.csv") -NoTypeInformation
Write-ColorOutput "  ✓ certificates.csv" -ForegroundColor Green

# ZIP圧縮
Write-ColorOutput "`n圧縮中..." -ForegroundColor Yellow
$zipPath = "$outputDir.zip"
Compress-Archive -Path $outputDir -DestinationPath $zipPath -Force
Remove-Item $outputDir -Recurse -Force

# 完了
Write-ColorOutput @"

╔═══════════════════════════════════════════════════════╗
║                                                       ║
║        ✓ 診断情報収集完了                            ║
║                                                       ║
╚═══════════════════════════════════════════════════════╝

出力ファイル: $zipPath
サイズ: $([math]::Round((Get-Item $zipPath).Length / 1KB, 2)) KB

このファイルをGitHub Issuesに添付してください。

"@ -ForegroundColor Green

# ファイルをExplorerで開く
explorer.exe "/select,$zipPath"




