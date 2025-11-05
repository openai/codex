# -*- coding: utf-8 -*-
"""
アーキテクチャ図のPNG生成（SNS用）
Chrome DevTools MCPサーバーまたはPlaywrightを使用
"""

import subprocess
import sys
from pathlib import Path

def generate_png_with_chrome(svg_path: Path, png_path: Path, width: int = 1200, height: int = 675):
    """Chrome DevToolsでPNG生成"""
    print(f"Generating PNG: {png_path}")
    print(f"Size: {width}x{height}")
    print("")
    
    # HTMLビューアーを開く
    html_viewer = Path("zapabob/scripts/svg-to-png-simple.html").absolute()
    svg_abs = svg_path.absolute()
    
    print("Method 1: Manual (Recommended)")
    print(f"1. Opening Chrome with: file:///{html_viewer}?svg=../{svg_path.name}")
    print(f"2. Press F12 → Ctrl+Shift+M (Device Toolbar)")
    print(f"3. Set size to {width} x {height}")
    print(f"4. Ctrl+Shift+P → 'Capture screenshot'")
    print(f"5. Save as: {png_path.name}")
    print("")
    
    # Chromeを開く
    try:
        chrome_path = "chrome"  # または "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe"
        url = f"file:///{html_viewer}?svg=../{svg_path.name}"
        subprocess.Popen([chrome_path, url])
        print("[OK] Chrome opened")
    except Exception as e:
        print(f"[INFO] Please open manually: {html_viewer}")
    
    print("")
    print("Alternative: Online conversion")
    print(f"Upload {svg_path} to https://cloudconvert.com/svg-to-png")

def main():
    # アーキテクチャ図
    arch_svg = Path("zapabob/docs/codex-architecture-current.svg")
    arch_png = Path("zapabob/docs/codex-architecture-sns.png")
    
    # リポジトリ構造図
    repo_svg = Path("zapabob/docs/repository-structure.svg")
    repo_png = Path("zapabob/docs/repository-structure-sns.png")
    
    if arch_svg.exists():
        print("=" * 60)
        print("Architecture Diagram (SNS用)")
        print("=" * 60)
        generate_png_with_chrome(arch_svg, arch_png, 1200, 675)
    
    print("")
    
    if repo_svg.exists():
        print("=" * 60)
        print("Repository Structure Diagram (SNS用)")
        print("=" * 60)
        generate_png_with_chrome(repo_svg, repo_png, 1200, 627)

if __name__ == "__main__":
    main()

