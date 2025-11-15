# SVG to ICO Converter using ImageMagick
param(
    [string]$SvgPath,
    [string]$OutputPath
)

Write-Host "Converting SVG to ICO: $SvgPath -> $OutputPath" -ForegroundColor Cyan

# Check if ImageMagick is available
$magickCmd = Get-Command magick -ErrorAction SilentlyContinue
if (-not $magickCmd) {
    Write-Host "Error: ImageMagick (magick) command not found." -ForegroundColor Red
    Write-Host "Please install ImageMagick:" -ForegroundColor Yellow
    Write-Host "  choco install imagemagick -y" -ForegroundColor White
    Write-Host "  OR run: .\scripts\install-imagemagick.ps1" -ForegroundColor White
    exit 1
}

# Check if input file exists
if (-not (Test-Path $SvgPath)) {
    Write-Host "Error: SVG file not found: $SvgPath" -ForegroundColor Red
    exit 1
}

try {
    # Create temporary directory for multiple sizes
    $tempDir = Join-Path $env:TEMP "codex-icon-temp"
    New-Item -ItemType Directory -Force -Path $tempDir | Out-Null
    
    # Generate multiple sizes for ICO
    $sizes = @(16, 32, 48, 256)
    $tempFiles = @()
    
    foreach ($size in $sizes) {
        $tempPng = Join-Path $tempDir "icon-${size}x${size}.png"
        
        # Convert SVG to PNG at specific size using ImageMagick
        Write-Host "  Generating ${size}x${size} icon..." -ForegroundColor Gray
        & magick convert -background none -density 300 -resize "${size}x${size}" "$SvgPath" "$tempPng"
        
        if (Test-Path $tempPng) {
            $tempFiles += $tempPng
        } else {
            Write-Host "  Warning: Failed to generate ${size}x${size} icon" -ForegroundColor Yellow
        }
    }
    
    if ($tempFiles.Count -eq 0) {
        Write-Host "Error: Failed to generate any icon sizes" -ForegroundColor Red
        Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
        exit 1
    }
    
    # Combine multiple PNGs into ICO format
    Write-Host "  Combining icons into ICO format..." -ForegroundColor Gray
    $icoArgs = $tempFiles + @($OutputPath)
    & magick convert $icoArgs
    
    if (Test-Path $OutputPath) {
        $fileSize = (Get-Item $OutputPath).Length / 1KB
        Write-Host "ICO file created successfully: $OutputPath ($([math]::Round($fileSize, 2)) KB)" -ForegroundColor Green
    } else {
        Write-Host "Error: ICO file was not created" -ForegroundColor Red
        Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
        exit 1
    }
    
    # Cleanup temporary files
    Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    
} catch {
    Write-Host "Error: $_" -ForegroundColor Red
    Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    exit 1
}

