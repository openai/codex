# PowerShell Script Encoding Fixer
# UTF-8 BOMなし → UTF-8 BOM付きに変換

<#
.SYNOPSIS
    PowerShellスクリプトをUTF-8 BOM付きに変換

.DESCRIPTION
    Windows PowerShellは UTF-8 BOMなしのファイルを正しく読めないため、
    すべての .ps1 ファイルを UTF-8 BOM付きに変換します
#>

param(
    [string]$Path = $PSScriptRoot
)

Write-Host "PowerShell Script Encoding Fixer" -ForegroundColor Cyan
Write-Host "=================================" -ForegroundColor Cyan
Write-Host ""

# すべての .ps1 ファイルを取得
$scripts = Get-ChildItem -Path $Path -Filter "*.ps1" -Recurse

Write-Host "Found $($scripts.Count) PowerShell scripts" -ForegroundColor Yellow
Write-Host ""

$fixed = 0
$skipped = 0

foreach ($script in $scripts) {
    Write-Host "Processing: $($script.Name)" -ForegroundColor Gray
    
    try {
        # ファイルを読み込み（UTF-8として）
        $content = Get-Content $script.FullName -Raw -Encoding UTF8
        
        # UTF-8 BOM付きで保存
        $utf8BOM = New-Object System.Text.UTF8Encoding $true
        [System.IO.File]::WriteAllText($script.FullName, $content, $utf8BOM)
        
        Write-Host "  -> Fixed (UTF-8 with BOM)" -ForegroundColor Green
        $fixed++
        
    } catch {
        Write-Host "  -> Error: $_" -ForegroundColor Red
        $skipped++
    }
}

Write-Host ""
Write-Host "=================================" -ForegroundColor Cyan
Write-Host "Fixed:   $fixed files" -ForegroundColor Green
Write-Host "Skipped: $skipped files" -ForegroundColor Yellow
Write-Host ""
Write-Host "Done! You can now run the scripts with Windows PowerShell." -ForegroundColor Green

