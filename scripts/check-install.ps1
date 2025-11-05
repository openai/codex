# Codex Global Installation Checker
# グローバルインストール確認スクリプト

Write-Host "🔍 Codex Installation Check" -ForegroundColor Cyan
Write-Host "=" * 60

# Check cargo bin directory
$cargoBin = "$env:USERPROFILE\.cargo\bin"
Write-Host "`n📂 Cargo bin directory: $cargoBin"

if (Test-Path $cargoBin\codex.exe) {
    Write-Host "✅ codex.exe found!" -ForegroundColor Green
    
    # Get file info
    $fileInfo = Get-Item $cargoBin\codex.exe
    Write-Host "   Size: $([math]::Round($fileInfo.Length / 1MB, 2)) MB"
    Write-Host "   Modified: $($fileInfo.LastWriteTime)"
    
    # Check version
    Write-Host "`n📦 Version check:"
    & $cargoBin\codex.exe --version
    
    # Check if in PATH
    Write-Host "`n🔗 PATH check:"
    $pathCheck = Get-Command codex -ErrorAction SilentlyContinue
    if ($pathCheck) {
        Write-Host "✅ codex is in PATH" -ForegroundColor Green
        Write-Host "   Location: $($pathCheck.Source)"
    } else {
        Write-Host "⚠️  codex not in PATH. Add $cargoBin to your PATH" -ForegroundColor Yellow
    }
    
    Write-Host "`n🎉 Installation successful!" -ForegroundColor Green
    
} else {
    Write-Host "❌ codex.exe not found in $cargoBin" -ForegroundColor Red
    Write-Host "   Run: cargo install --path cli --force"
}

Write-Host "`n" + ("=" * 60)

