# Codex AI Driver - 構文チェックスクリプト
# 型エラー・警告を確認（ビルドはしない）

$ErrorActionPreference = "Continue"

Write-Host @"

╔═══════════════════════════════════════════╗
║  AI Driver Syntax Checker v0.4.1         ║
╚═══════════════════════════════════════════╝

"@ -ForegroundColor Cyan

# WDK Include paths
$wdkPath = "C:\Program Files (x86)\Windows Kits\10"
$wdkVersion = "10.0.26100.0"
$includePaths = @(
    "$wdkPath\Include\$wdkVersion\km",
    "$wdkPath\Include\$wdkVersion\shared",
    "$wdkPath\Include\$wdkVersion\um"
)

# Source files
$sourceFiles = @(
    "ai_driver.c",
    "ai_driver_ioctl.c",
    "ioctl_handlers.c",
    "gpu_integration.c",
    "nvapi_bridge.c",
    "dx12_compute.c"
)

# Compiler flags
$compilerFlags = @(
    "/c",                    # Compile only (no link)
    "/nologo",               # No logo
    "/W4",                   # Warning level 4
    "/WX",                   # Treat warnings as errors
    "/Zc:wchar_t",          # wchar_t is native type
    "/D_AMD64_",            # AMD64 architecture
    "/D_WIN64",             # Win64
    "/DKERNEL_MODE=1"       # Kernel mode
)

# Add include paths
foreach ($includePath in $includePaths) {
    if (Test-Path $includePath) {
        $compilerFlags += "/I`"$includePath`""
    } else {
        Write-Host "⚠ Include path not found: $includePath" -ForegroundColor Yellow
    }
}

# Check each source file
$totalFiles = $sourceFiles.Count
$successCount = 0
$failureCount = 0
$warningCount = 0

Write-Host "`n構文チェック開始...`n" -ForegroundColor Yellow

foreach ($sourceFile in $sourceFiles) {
    Write-Host "[$($sourceFiles.IndexOf($sourceFile) + 1)/$totalFiles] $sourceFile" -ForegroundColor Gray
    
    if (-not (Test-Path $sourceFile)) {
        Write-Host "  ✗ ファイルが見つかりません" -ForegroundColor Red
        $failureCount++
        continue
    }
    
    # Run compiler in syntax check mode
    $output = & cl @compilerFlags $sourceFile 2>&1
    $exitCode = $LASTEXITCODE
    
    # Analyze output
    $errors = $output | Where-Object { $_ -match "error C" }
    $warnings = $output | Where-Object { $_ -match "warning C" }
    
    if ($exitCode -eq 0) {
        Write-Host "  ✓ 構文OK（エラー: 0, 警告: 0）" -ForegroundColor Green
        $successCount++
    } elseif ($errors.Count -gt 0) {
        Write-Host "  ✗ エラー: $($errors.Count)" -ForegroundColor Red
        $errors | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
        $failureCount++
    } elseif ($warnings.Count -gt 0) {
        Write-Host "  ⚠ 警告: $($warnings.Count)" -ForegroundColor Yellow
        $warnings | ForEach-Object { Write-Host "    $_" -ForegroundColor Yellow }
        $warningCount++
    }
}

# Clean up object files
Remove-Item *.obj -ErrorAction SilentlyContinue

# Summary
Write-Host @"

╔═══════════════════════════════════════════╗
║  構文チェック結果                        ║
╚═══════════════════════════════════════════╝

"@ -ForegroundColor Cyan

Write-Host "総ファイル数: $totalFiles" -ForegroundColor White
Write-Host "成功: $successCount" -ForegroundColor Green
Write-Host "警告: $warningCount" -ForegroundColor Yellow
Write-Host "エラー: $failureCount" -ForegroundColor Red

if ($failureCount -eq 0 -and $warningCount -eq 0) {
    Write-Host "`n✓ すべてのファイルが型エラー・警告ゼロです！" -ForegroundColor Green
    Write-Host "本番環境レベルのコード品質を達成しています。" -ForegroundColor Green
    exit 0
} elseif ($failureCount -eq 0) {
    Write-Host "`n⚠ 警告がありますが、エラーはありません" -ForegroundColor Yellow
    exit 0
} else {
    Write-Host "`n✗ エラーがあります。修正が必要です。" -ForegroundColor Red
    exit 1
}

