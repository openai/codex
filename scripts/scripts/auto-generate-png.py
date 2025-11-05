# -*- coding: utf-8 -*-
"""
SVGをPNGに自動変換（Playwright使用）
SNS用サイズ: 1200x675
"""

import asyncio
import sys
from pathlib import Path

async def convert_svg_to_png_auto(svg_file: Path, png_file: Path, width: int = 1200, height: int = 675):
    """PlaywrightでSVG→PNG自動変換"""
    try:
        from playwright.async_api import async_playwright
    except ImportError:
        print("[ERROR] Playwright not installed")
        print("Install: pip install playwright")
        print("Run: python -m playwright install chromium")
        return False
    
    print(f"Converting {svg_file.name} to PNG ({width}x{height})...")
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(
            viewport={'width': width, 'height': height},
            device_scale_factor=2  # Retina quality
        )
        page = await context.new_page()
        
        # SVGファイルを開く
        svg_url = f"file:///{svg_file.absolute().as_posix()}"
        await page.goto(svg_url, wait_until="networkidle", timeout=10000)
        
        # スクリーンショット（透過背景を白に）
        await page.emulate_media(color_scheme="light")
        await page.screenshot(
            path=str(png_file),
            full_page=False,
            omit_background=False
        )
        
        await browser.close()
    
    print(f"[OK] PNG created: {png_file}")
    return True

async def main():
    """アーキテクチャ図2つをPNG化"""
    
    # 1. アーキテクチャ図 (Twitter/X用)
    arch_svg = Path("zapabob/docs/codex-architecture-current.svg")
    arch_png = Path("zapabob/docs/codex-architecture-sns.png")
    
    if arch_svg.exists():
        print("=" * 60)
        print("Architecture Diagram (Twitter/X: 1200x675)")
        print("=" * 60)
        try:
            await convert_svg_to_png_auto(arch_svg, arch_png, 1200, 675)
        except Exception as e:
            print(f"[ERROR] {e}")
    
    # 2. リポジトリ構造図 (LinkedIn用)
    repo_svg = Path("zapabob/docs/repository-structure.svg")
    repo_png = Path("zapabob/docs/repository-structure-sns.png")
    
    if repo_svg.exists():
        print("")
        print("=" * 60)
        print("Repository Structure (LinkedIn: 1200x627)")
        print("=" * 60)
        try:
            await convert_svg_to_png_auto(repo_svg, repo_png, 1200, 627)
        except Exception as e:
            print(f"[ERROR] {e}")
    
    print("")
    print("[DONE] PNG generation complete!")
    print(f"Output: zapabob/docs/*-sns.png")

if __name__ == "__main__":
    asyncio.run(main())

