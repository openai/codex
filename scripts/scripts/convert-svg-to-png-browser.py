#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Convert SVG to PNG using browser-based rendering (Playwright).
This is the most reliable method for Windows without Cairo dependencies.

Usage:
    py -3 convert-svg-to-png-browser.py [--width WIDTH] [--height HEIGHT]

Requirements:
    - playwright (pip install playwright)
    - playwright install chromium (one-time setup)
"""

import sys
import os
from pathlib import Path
import subprocess
import argparse
import asyncio

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


def install_playwright_browsers() -> bool:
    """Install Playwright browsers (Chromium)."""
    print("📦 Installing Playwright Chromium browser...")
    try:
        subprocess.run(
            [sys.executable, "-m", "playwright", "install", "chromium"],
            check=True
        )
        print("✅ Chromium browser installed successfully")
        return True
    except subprocess.CalledProcessError as e:
        print(f"❌ Failed to install Chromium: {e}")
        return False


async def convert_svg_to_png_async(
    svg_path: Path,
    png_path: Path,
    width: int = 2400,
    height: int = 1600
) -> bool:
    """
    Convert SVG to PNG using Playwright browser rendering.
    
    Args:
        svg_path: Path to input SVG file
        png_path: Path to output PNG file
        width: Viewport width in pixels
        height: Viewport height in pixels
    
    Returns:
        True if successful, False otherwise
    """
    print("🌐 Using Playwright (Chromium) for SVG to PNG conversion...")
    
    try:
        from playwright.async_api import async_playwright
    except ImportError:
        print("❌ Playwright not found. Installing...")
        if not install_package("playwright"):
            return False
        
        # Install browsers
        if not install_playwright_browsers():
            return False
        
        try:
            from playwright.async_api import async_playwright
        except ImportError:
            print("❌ Failed to import Playwright after installation")
            return False
    
    try:
        # Read SVG content
        svg_content = svg_path.read_text(encoding='utf-8')
        
        # Create HTML wrapper for SVG
        html_content = f"""
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {{
            margin: 0;
            padding: 20px;
            background: white;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
        }}
        svg {{
            max-width: 100%;
            height: auto;
        }}
    </style>
</head>
<body>
    {svg_content}
</body>
</html>
"""
        
        print(f"🔧 Rendering SVG with browser...")
        print(f"   Viewport: {width}x{height} pixels")
        
        async with async_playwright() as p:
            # Launch headless browser
            browser = await p.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": width, "height": height})
            
            # Set HTML content
            await page.set_content(html_content)
            
            # Wait for SVG to render
            await page.wait_for_load_state("networkidle")
            await asyncio.sleep(1)  # Extra time for SVG rendering
            
            # Take screenshot
            await page.screenshot(
                path=str(png_path),
                full_page=True,
                type="png"
            )
            
            await browser.close()
        
        print(f"✅ PNG saved to: {png_path}")
        print(f"📊 File size: {png_path.stat().st_size / 1024:.2f} KB")
        return True
    
    except Exception as e:
        print(f"❌ Playwright conversion failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description="Convert Codex architecture SVG to PNG using browser rendering"
    )
    parser.add_argument(
        "--width",
        type=int,
        default=2400,
        help="Viewport width in pixels (default: 2400)"
    )
    parser.add_argument(
        "--height",
        type=int,
        default=1600,
        help="Viewport height in pixels (default: 1600)"
    )
    args = parser.parse_args()
    
    print("🎨 Codex v0.48.0 Architecture SVG → PNG Converter (Browser)")
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
    
    # Run async conversion
    success = asyncio.run(
        convert_svg_to_png_async(svg_path, png_path, args.width, args.height)
    )
    
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
        print("   py -3 convert-svg-to-png-browser.py --width 1920 --height 1080  # Full HD")
        print("   py -3 convert-svg-to-png-browser.py --width 2560 --height 1440  # 2K")
        print("   py -3 convert-svg-to-png-browser.py --width 3840 --height 2160  # 4K")
        return 0
    else:
        print()
        print("=" * 60)
        print("❌ FAILED to convert SVG to PNG.")
        print("💡 Try installing dependencies manually:")
        print("   pip install playwright")
        print("   playwright install chromium")
        return 1


if __name__ == "__main__":
    sys.exit(main())

