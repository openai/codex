# Mermaid図をSVG/PNG形式に変換するスクリプト

param(
    [string]$InputFile = "zapabob/docs/codex-architecture-current.mmd",
    [string]$OutputDir = "zapabob/docs"
)

Write-Host "🔄 Generating images from Mermaid diagrams..." -ForegroundColor Cyan

# mermaid-cliを使用してSVG生成
$baseName = [System.IO.Path]::GetFileNameWithoutExtension($InputFile)
$svgOutput = Join-Path $OutputDir "$baseName.svg"
$pngOutput = Join-Path $OutputDir "$baseName.png"

# 方法1: mermaid.ink API（オンライン）
try {
    $mermaidContent = Get-Content $InputFile -Raw
    # ```mermaid を除去
    $mermaidContent = $mermaidContent -replace '```mermaid\s*', '' -replace '```\s*$', ''
    
    # mermaid.inkのエンコード
    $encoded = [System.Web.HttpUtility]::UrlEncode($mermaidContent)
    $apiUrl = "https://mermaid.ink/svg/$encoded"
    
    Write-Host "📥 Downloading SVG from mermaid.ink..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $apiUrl -OutFile $svgOutput -ErrorAction Stop
    Write-Host "✅ SVG created: $svgOutput" -ForegroundColor Green
    
    # PNG変換はSVGの情報を表示
    Write-Host "ℹ️ PNG conversion requires additional tools (ImageMagick, Inkscape, or cairosvg)" -ForegroundColor Gray
    Write-Host "SVG file can be viewed in browsers and converted manually if needed" -ForegroundColor Gray
    
} catch {
    Write-Host "❌ Error: $_" -ForegroundColor Red
    Write-Host "" -ForegroundColor White
    Write-Host "Alternative: Use online tool" -ForegroundColor Cyan
    Write-Host "1. Open https://mermaid.live/" -ForegroundColor White
    Write-Host "2. Paste content from $InputFile" -ForegroundColor White
    Write-Host "3. Export as SVG/PNG" -ForegroundColor White
    exit 1
}

Write-Host "" -ForegroundColor White
Write-Host "✨ Generation complete!" -ForegroundColor Green
Write-Host "SVG: $svgOutput" -ForegroundColor White

