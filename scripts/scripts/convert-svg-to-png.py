#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Convert SVG architecture diagram to PNG with high quality.

Usage:
    py -3 convert-svg-to-png.py [--width WIDTH] [--height HEIGHT]

Requirements:
    - cairosvg (pip install cairosvg)
    - OR Pillow + svglib (pip install Pillow svglib reportlab)
"""

import sys
import os
from pathlib import Path
import subprocess
import argparse

# Force UTF-8 output on Windows
if sys.platform == 'win32':
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')


def install_package(package_name: str) -> bool:
    """Install a Python package via pip."""
    print(f"📦 Installing {package_name}...")
    try:
        subprocess.run(
            [sys.executable, "-m", "pip", "install", package_name],
            check=True,
            capture_output=True
        )
        print(f"✅ {package_name} installed successfully")
        return True
    except subprocess.CalledProcessError as e:
        print(f"❌ Failed to install {package_name}: {e}")
        return False


def convert_via_cairosvg(svg_path: Path, png_path: Path, width: int = None, height: int = None) -> bool:
    """
    Convert SVG to PNG using cairosvg (best quality).
    
    Args:
        svg_path: Path to input SVG file
        png_path: Path to output PNG file
        width: Output width in pixels (optional)
        height: Output height in pixels (optional)
    
    Returns:
        True if successful, False otherwise
    """
    print("🖼️  Using cairosvg for SVG to PNG conversion...")
    
    try:
        import cairosvg
    except ImportError:
        print("❌ cairosvg not found. Installing...")
        if not install_package("cairosvg"):
            return False
        try:
            import cairosvg
        except ImportError:
            print("❌ Failed to import cairosvg after installation")
            return False
    
    try:
        # Read SVG content
        svg_content = svg_path.read_text(encoding='utf-8')
        
        # Convert with specified dimensions
        kwargs = {}
        if width:
            kwargs['output_width'] = width
        if height:
            kwargs['output_height'] = height
        
        print(f"🔧 Converting SVG to PNG...")
        if kwargs:
            print(f"   Dimensions: {width or 'auto'}x{height or 'auto'} pixels")
        
        cairosvg.svg2png(
            bytestring=svg_content.encode('utf-8'),
            write_to=str(png_path),
            **kwargs
        )
        
        print(f"✅ PNG saved to: {png_path}")
        print(f"📊 File size: {png_path.stat().st_size / 1024:.2f} KB")
        return True
    
    except Exception as e:
        print(f"❌ cairosvg conversion failed: {e}")
        return False


def convert_via_pillow(svg_path: Path, png_path: Path, width: int = None, height: int = None) -> bool:
    """
    Convert SVG to PNG using Pillow + svglib (fallback).
    
    Args:
        svg_path: Path to input SVG file
        png_path: Path to output PNG file
        width: Output width in pixels (optional)
        height: Output height in pixels (optional)
    
    Returns:
        True if successful, False otherwise
    """
    print("🖼️  Using Pillow + svglib for SVG to PNG conversion...")
    
    try:
        from svglib.svglib import svg2rlg
        from reportlab.graphics import renderPM
    except ImportError:
        print("❌ svglib/reportlab not found. Installing...")
        if not install_package("svglib") or not install_package("reportlab"):
            return False
        try:
            from svglib.svglib import svg2rlg
            from reportlab.graphics import renderPM
        except ImportError:
            print("❌ Failed to import svglib/reportlab after installation")
            return False
    
    try:
        # Convert SVG to ReportLab drawing
        print(f"🔧 Converting SVG to PNG...")
        drawing = svg2rlg(str(svg_path))
        
        if drawing is None:
            print("❌ Failed to parse SVG file")
            return False
        
        # Scale if dimensions specified
        if width and height:
            scale_x = width / drawing.width
            scale_y = height / drawing.height
            scale = min(scale_x, scale_y)
            drawing.width *= scale
            drawing.height *= scale
            drawing.scale(scale, scale)
            print(f"   Dimensions: {int(drawing.width)}x{int(drawing.height)} pixels")
        
        # Render to PNG
        renderPM.drawToFile(drawing, str(png_path), fmt='PNG')
        
        print(f"✅ PNG saved to: {png_path}")
        print(f"📊 File size: {png_path.stat().st_size / 1024:.2f} KB")
        return True
    
    except Exception as e:
        print(f"❌ Pillow conversion failed: {e}")
        return False


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Convert Codex architecture SVG to PNG"
    )
    parser.add_argument(
        "--width",
        type=int,
        help="Output width in pixels (default: auto)"
    )
    parser.add_argument(
        "--height",
        type=int,
        help="Output height in pixels (default: auto)"
    )
    parser.add_argument(
        "--method",
        choices=["cairosvg", "pillow", "auto"],
        default="auto",
        help="Conversion method (default: auto)"
    )
    args = parser.parse_args()
    
    print("🎨 Codex v0.48.0 Architecture SVG → PNG Converter")
    print("=" * 60)
    
    # Paths
    repo_root = Path(__file__).parent.parent.parent
    svg_path = repo_root / "zapabob" / "docs" / "codex-v0.48.0-architecture.svg"
    png_path = repo_root / "zapabob" / "docs" / "codex-v0.48.0-architecture.png"
    
    # Check input file
    if not svg_path.exists():
        print(f"❌ Input file not found: {svg_path}")
        return 1
    
    print(f"📄 Input:  {svg_path}")
    print(f"💾 Output: {png_path}")
    print()
    
    # Try conversion
    success = False
    
    if args.method == "cairosvg":
        success = convert_via_cairosvg(svg_path, png_path, args.width, args.height)
    elif args.method == "pillow":
        success = convert_via_pillow(svg_path, png_path, args.width, args.height)
    else:  # auto
        success = convert_via_cairosvg(svg_path, png_path, args.width, args.height)
        if not success:
            print("\n🔄 Falling back to Pillow method...")
            success = convert_via_pillow(svg_path, png_path, args.width, args.height)
    
    if success:
        print()
        print("=" * 60)
        print("🎉 SUCCESS! PNG conversion completed.")
        print(f"📂 Location: {png_path.relative_to(repo_root)}")
        print()
        print("📌 Usage:")
        print("  - Twitter/X: Upload PNG directly (better compatibility)")
        print("  - LinkedIn: High-quality image for posts")
        print("  - Presentation: Embed in slides/documents")
        print("  - Documentation: Add to wikis/docs")
        print()
        print("💡 For different sizes, use:")
        print("   py -3 convert-svg-to-png.py --width 1920 --height 1080  # Full HD")
        print("   py -3 convert-svg-to-png.py --width 2560 --height 1440  # 2K")
        print("   py -3 convert-svg-to-png.py --width 3840 --height 2160  # 4K")
        return 0
    else:
        print()
        print("=" * 60)
        print("❌ FAILED to convert SVG to PNG.")
        print("💡 Try installing dependencies manually:")
        print("   pip install cairosvg")
        print("   OR")
        print("   pip install svglib reportlab Pillow")
        return 1


if __name__ == "__main__":
    sys.exit(main())

