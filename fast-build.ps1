# 高速ビルドスクリプト
# 並列化 + LLDリンカー + インクリメンタルビルド

param(
    [switch]$Clean,
    [switch]$Release,
    [int]$Jobs = 16  # CPU cores数に応じて調整
)

$ErrorActionPreference = "Continue"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Rust Fast Build" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# System info
$cpuCores = (Get-WmiObject Win32_Processor).NumberOfLogicalProcessors
Write-Host "CPU Cores: $cpuCores" -ForegroundColor White
Write-Host "Parallel Jobs: $Jobs" -ForegroundColor White
Write-Host ""

Set-Location "codex-rs"

if ($Clean) {
    Write-Host "Cleaning build cache..." -ForegroundColor Yellow
    cargo clean
    Write-Host "Clean complete!" -ForegroundColor Green
    Write-Host ""
}

Write-Host "Starting fast build..." -ForegroundColor Yellow
Write-Host "  Package: codex-cli" -ForegroundColor Gray
Write-Host "  Profile: $(if ($Release) { 'release' } else { 'dev' })" -ForegroundColor Gray
Write-Host "  Jobs: $Jobs" -ForegroundColor Gray
Write-Host ""

$buildStart = Get-Date

if ($Release) {
    # リリースビルド（高速版プロファイル使用可能）
    cargo build --release -p codex-cli -j $Jobs
} else {
    # デバッグビルド（インクリメンタル有効）
    cargo build -p codex-cli -j $Jobs
}

$buildEnd = Get-Date
$duration = ($buildEnd - $buildStart).TotalSeconds

Set-Location ".."

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan

if ($LASTEXITCODE -eq 0) {
    $binaryPath = if ($Release) { "codex-rs\target\release\codex.exe" } else { "codex-rs\target\debug\codex.exe" }
    
    if (Test-Path $binaryPath) {
        $fileInfo = Get-Item $binaryPath
        Write-Host " BUILD SUCCESS!" -ForegroundColor Green
        Write-Host "========================================" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "Time: $([math]::Floor($duration / 60))m $([math]::Floor($duration % 60))s" -ForegroundColor White
        Write-Host "Size: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor White
        Write-Host "Binary: $binaryPath" -ForegroundColor White
        Write-Host ""
        
        if ($Release) {
            Write-Host "Next: .\install-phase4.ps1" -ForegroundColor Cyan
        }
    }
} else {
    Write-Host " BUILD FAILED!" -ForegroundColor Red
    Write-Host "========================================" -ForegroundColor Red
}

Write-Host ""

