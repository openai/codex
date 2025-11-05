# -*- coding: utf-8 -*-
"""SVG to PNG converter using browser (Playwright)"""

import sys
import asyncio
from pathlib import Path

async def convert_svg_to_png(svg_file: Path, png_file: Path):
    """Convert SVG to PNG using Playwright"""
    try:
        from playwright.async_api import async_playwright
    except ImportError:
        print("[ERROR] Playwright not installed")
        print("Install: pip install playwright")
        print("Then run: playwright install chromium")
        return False
    
    async with async_playwright() as p:
        browser = await p.chromium.launch()
        page = await browser.new_page()
        
        # Load SVG
        await page.goto(f"file:///{svg_file.absolute()}")
        await page.wait_for_load_state("networkidle")
        
        # Screenshot
        await page.screenshot(path=str(png_file), full_page=True)
        
        await browser.close()
    
    print(f"[OK] PNG: {png_file}")
    return True

def main():
    if len(sys.argv) < 2:
        print("Usage: python svg-to-png-browser.py <input.svg>")
        sys.exit(1)
    
    svg_file = Path(sys.argv[1])
    png_file = svg_file.parent / f"{svg_file.stem}.png"
    
    asyncio.run(convert_svg_to_png(svg_file, png_file))

if __name__ == "__main__":
    main()

