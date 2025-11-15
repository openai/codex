# SVG to ICO Converter using Python (Pillow + svglib)
param(
    [string]$SvgPath,
    [string]$OutputPath
)

Write-Host "Converting SVG to ICO using Python: $SvgPath -> $OutputPath" -ForegroundColor Cyan

# Check if input file exists
if (-not (Test-Path $SvgPath)) {
    Write-Host "Error: SVG file not found: $SvgPath" -ForegroundColor Red
    exit 1
}

# Python script to convert SVG to ICO
$pythonScript = @"
import sys
from pathlib import Path

try:
    from PIL import Image
    from svglib.svglib import svg2rlg
    from reportlab.graphics import renderPM
except ImportError as e:
    print(f"Error: Required libraries not installed: {e}")
    print("Please install: pip install Pillow svglib reportlab")
    sys.exit(1)

svg_path = r"$SvgPath"
output_path = r"$OutputPath"

try:
    # Convert SVG to ReportLab drawing
    print("Converting SVG to drawing...")
    drawing = svg2rlg(svg_path)
    
    if drawing is None:
        print("Error: Failed to parse SVG file")
        sys.exit(1)
    
    # Create temporary directory for multiple sizes
    import tempfile
    import os
    temp_dir = tempfile.mkdtemp()
    
    # Generate multiple sizes for ICO
    sizes = [16, 32, 48, 256]
    temp_files = []
    
    for size in sizes:
        temp_png = os.path.join(temp_dir, f"icon-{size}x{size}.png")
        
        # Scale drawing
        scale = size / max(drawing.width, drawing.height)
        scaled_drawing = drawing
        scaled_drawing.width = drawing.width * scale
        scaled_drawing.height = drawing.height * scale
        scaled_drawing.scale(scale, scale)
        
        # Render to PNG
        renderPM.drawToFile(scaled_drawing, temp_png, fmt='PNG')
        temp_files.append(temp_png)
        print(f"  Generated {size}x{size} icon")
    
    # Combine PNGs into ICO format
    print("Combining icons into ICO format...")
    images = [Image.open(f) for f in temp_files]
    
    # Save as ICO (Pillow supports multi-size ICO)
    images[0].save(
        output_path,
        format='ICO',
        sizes=[(img.width, img.height) for img in images]
    )
    
    # Cleanup
    import shutil
    shutil.rmtree(temp_dir)
    
    file_size = os.path.getsize(output_path) / 1024
    print(f"ICO file created successfully: {output_path} ({file_size:.2f} KB)")
    
except Exception as e:
    print(f"Error: {e}")
    import traceback
    traceback.print_exc()
    sys.exit(1)
"@

# Execute Python script
$pythonScript | py -3 - 2>&1

if ($LASTEXITCODE -ne 0) {
    Write-Host "Python conversion failed. Installing required packages..." -ForegroundColor Yellow
    py -3 -m pip install --quiet Pillow svglib reportlab 2>&1 | Out-Null
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Retrying conversion..." -ForegroundColor Yellow
        $pythonScript | py -3 - 2>&1
    } else {
        Write-Host "Error: Failed to install required Python packages" -ForegroundColor Red
        exit 1
    }
}

if (Test-Path $OutputPath) {
    Write-Host "ICO file created successfully: $OutputPath" -ForegroundColor Green
    exit 0
} else {
    Write-Host "Error: ICO file was not created" -ForegroundColor Red
    exit 1
}

