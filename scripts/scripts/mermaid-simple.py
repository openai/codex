# -*- coding: utf-8 -*-
"""Mermaid to SVG converter (kroki.io API)"""

import sys
import base64
import zlib
import requests
from pathlib import Path

def main():
    if len(sys.argv) < 2:
        print("Usage: python mermaid-simple.py <input.mmd>")
        sys.exit(1)
    
    mermaid_file = Path(sys.argv[1])
    
    # Read Mermaid content
    with open(mermaid_file, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Remove markdown code blocks
    content = content.replace('```mermaid', '').replace('```', '').strip()
    
    # Compress and encode
    compressed = zlib.compress(content.encode('utf-8'), level=9)
    encoded = base64.urlsafe_b64encode(compressed).decode('utf-8')
    
    # kroki API
    url = f"https://kroki.io/mermaid/svg/{encoded}"
    
    print(f"Downloading SVG from kroki.io...")
    response = requests.get(url, timeout=30)
    response.raise_for_status()
    
    # Save SVG
    output_svg = mermaid_file.parent / f"{mermaid_file.stem}.svg"
    with open(output_svg, 'wb') as f:
        f.write(response.content)
    
    print(f"[OK] SVG: {output_svg}")
    
    # Try PNG conversion
    try:
        import cairosvg
        output_png = mermaid_file.parent / f"{mermaid_file.stem}.png"
        cairosvg.svg2png(
            url=str(output_svg),
            write_to=str(output_png),
            output_width=2400
        )
        print(f"[OK] PNG: {output_png}")
    except ImportError:
        print("[INFO] Install 'pip install cairosvg' for PNG conversion")
    except Exception as e:
        print(f"[WARN] PNG failed: {e}")

if __name__ == "__main__":
    main()

