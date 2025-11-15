# SVG to ICO Converter using Python (Pillow only - simple approach)
param(
    [string]$SvgPath,
    [string]$OutputPath
)

Write-Host "Converting SVG to ICO using Python (Pillow): $SvgPath -> $OutputPath" -ForegroundColor Cyan

# Check if input file exists
if (-not (Test-Path $SvgPath)) {
    Write-Host "Error: SVG file not found: $SvgPath" -ForegroundColor Red
    exit 1
}

# Python script to convert SVG to ICO (using Pillow to create simple icon from SVG colors)
$pythonScript = @"
import sys
from pathlib import Path
from PIL import Image, ImageDraw
import xml.etree.ElementTree as ET
import re

svg_path = r"$SvgPath"
output_path = r"$OutputPath"

try:
    # Read SVG file
    with open(svg_path, 'r', encoding='utf-8') as f:
        svg_content = f.read()
    
    # Extract background color from SVG
    bg_color = "#0061a4"  # Default
    if 'fill="#' in svg_content:
        match = re.search(r'fill="([^"]+)"', svg_content)
        if match:
            color_value = match.group(1)
            # Skip "none" and other non-hex values
            if color_value.startswith('#') and len(color_value) == 7:
                bg_color = color_value
    
    # Also check for fill in rect element
    if bg_color == "#0061a4":  # Still default
        match = re.search(r'<rect[^>]*fill="([^"]+)"', svg_content)
        if match:
            color_value = match.group(1)
            if color_value.startswith('#') and len(color_value) == 7:
                bg_color = color_value
    
    # Convert hex color to RGB
    def hex_to_rgb(hex_color):
        hex_color = hex_color.lstrip('#')
        if len(hex_color) != 6:
            return (0, 97, 164)  # Default blue
        try:
            return tuple(int(hex_color[i:i+2], 16) for i in (0, 2, 4))
        except ValueError:
            return (0, 97, 164)  # Default blue
    
    bg_rgb = hex_to_rgb(bg_color)
    
    # Create temporary directory for multiple sizes
    import tempfile
    import os
    temp_dir = tempfile.mkdtemp()
    
    # Generate multiple sizes for ICO
    sizes = [16, 32, 48, 256]
    temp_files = []
    
    for size in sizes:
        temp_png = os.path.join(temp_dir, f"icon-{size}x{size}.png")
        
        # Create image with background color
        img = Image.new('RGB', (size, size), bg_rgb)
        draw = ImageDraw.Draw(img)
        
        # Draw simple Codex logo (circle with "C")
        margin = size // 8
        circle_size = size - margin * 2
        
        # Draw circle
        draw.ellipse(
            [margin, margin, margin + circle_size, margin + circle_size],
            outline='white',
            width=max(1, size // 32)
        )
        
        # Draw "C" text (simplified - just a circle arc for small sizes)
        if size >= 32:
            from PIL import ImageFont
            try:
                # Try to use system font
                font_size = size // 2
                font = ImageFont.truetype("arial.ttf", font_size)
            except:
                font = ImageFont.load_default()
            
            text = "C"
            bbox = draw.textbbox((0, 0), text, font=font)
            text_width = bbox[2] - bbox[0]
            text_height = bbox[3] - bbox[1]
            x = (size - text_width) // 2
            y = (size - text_height) // 2
            draw.text((x, y), text, fill='white', font=font)
        else:
            # For small sizes, just draw a white circle
            draw.ellipse(
                [size // 3, size // 3, size * 2 // 3, size * 2 // 3],
                outline='white',
                width=1
            )
        
        img.save(temp_png)
        temp_files.append(temp_png)
        print(f"  Generated {size}x{size} icon")
    
    # Combine PNGs into ICO format
    print("Combining icons into ICO format...")
    images = []
    for f in temp_files:
        img = Image.open(f)
        images.append(img.copy())  # Make a copy to avoid file lock
        img.close()  # Close the file immediately
    
    # Save as ICO (Pillow supports multi-size ICO)
    try:
        images[0].save(
            output_path,
            format='ICO',
            sizes=[(img.width, img.height) for img in images]
        )
    except Exception as e:
        # Fallback: save first image as ICO
        print(f"Warning: Multi-size ICO failed ({e}), saving single size...")
        images[0].save(output_path, format='ICO')
    
    # Cleanup images
    for img in images:
        img.close()
    
    # Cleanup temporary directory (with retry for file locks)
    import shutil
    import time
    max_retries = 3
    for i in range(max_retries):
        try:
            shutil.rmtree(temp_dir)
            break
        except PermissionError:
            if i < max_retries - 1:
                time.sleep(0.5)
            else:
                print(f"Warning: Could not remove temp directory: {temp_dir}")
    
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
    Write-Host "Python conversion failed. Installing Pillow..." -ForegroundColor Yellow
    py -3 -m pip install --quiet Pillow 2>&1 | Out-Null
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Retrying conversion..." -ForegroundColor Yellow
        $pythonScript | py -3 - 2>&1
    } else {
        Write-Host "Error: Failed to install Pillow" -ForegroundColor Red
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

