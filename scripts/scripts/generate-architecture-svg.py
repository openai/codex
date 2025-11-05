#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Generate SVG from Mermaid diagram using Mermaid CLI (mmdc) or online API.

Usage:
    py -3 generate-architecture-svg.py

Requirements:
    - requests (pip install requests)
    - OR mermaid-cli installed globally (npm install -g @mermaid-js/mermaid-cli)
"""

import base64
import json
import sys
import os
from pathlib import Path
import subprocess

# Force UTF-8 output on Windows
if sys.platform == 'win32':
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

try:
    import requests
except ImportError:
    print("❌ requests not installed. Installing...")
    subprocess.run([sys.executable, "-m", "pip", "install", "requests"], check=True)
    import requests


def generate_svg_via_api(mermaid_code: str, output_path: Path) -> bool:
    """
    Generate SVG using Mermaid.ink API (online service).
    
    Args:
        mermaid_code: Mermaid diagram code
        output_path: Path to save SVG file
    
    Returns:
        True if successful, False otherwise
    """
    print("🌐 Using Mermaid.ink API for SVG generation...")
    
    # Encode Mermaid code to base64
    graph_bytes = mermaid_code.encode('utf-8')
    base64_bytes = base64.b64encode(graph_bytes)
    base64_string = base64_bytes.decode('ascii')
    
    # Mermaid.ink API endpoint
    url = f"https://mermaid.ink/svg/{base64_string}"
    
    print(f"📡 Fetching SVG from {url[:80]}...")
    
    try:
        response = requests.get(url, timeout=30)
        response.raise_for_status()
        
        # Save SVG
        output_path.write_bytes(response.content)
        print(f"✅ SVG saved to: {output_path}")
        print(f"📊 File size: {len(response.content) / 1024:.2f} KB")
        return True
    
    except requests.RequestException as e:
        print(f"❌ API request failed: {e}")
        return False


def generate_svg_via_cli(mermaid_code: str, output_path: Path, input_path: Path) -> bool:
    """
    Generate SVG using Mermaid CLI (mmdc).
    
    Args:
        mermaid_code: Mermaid diagram code (not used, reads from input_path)
        output_path: Path to save SVG file
        input_path: Path to input .mmd file
    
    Returns:
        True if successful, False otherwise
    """
    print("🖥️  Using Mermaid CLI (mmdc) for SVG generation...")
    
    # Check if mmdc is installed
    try:
        subprocess.run(["mmdc", "--version"], check=True, capture_output=True)
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("❌ Mermaid CLI (mmdc) not found. Install with:")
        print("   npm install -g @mermaid-js/mermaid-cli")
        return False
    
    # Generate SVG
    try:
        cmd = [
            "mmdc",
            "-i", str(input_path),
            "-o", str(output_path),
            "-b", "transparent",
            "-t", "default"
        ]
        print(f"🔧 Running: {' '.join(cmd)}")
        subprocess.run(cmd, check=True)
        
        print(f"✅ SVG saved to: {output_path}")
        print(f"📊 File size: {output_path.stat().st_size / 1024:.2f} KB")
        return True
    
    except subprocess.CalledProcessError as e:
        print(f"❌ Mermaid CLI failed: {e}")
        return False


def main():
    """Main entry point."""
    print("🎨 Codex v0.48.0 Architecture SVG Generator")
    print("=" * 60)
    
    # Paths
    repo_root = Path(__file__).parent.parent.parent
    mmd_path = repo_root / "zapabob" / "docs" / "codex-v0.48.0-architecture.mmd"
    svg_path = repo_root / "zapabob" / "docs" / "codex-v0.48.0-architecture.svg"
    
    # Check input file
    if not mmd_path.exists():
        print(f"❌ Input file not found: {mmd_path}")
        return 1
    
    print(f"📄 Input:  {mmd_path}")
    print(f"💾 Output: {svg_path}")
    print()
    
    # Read Mermaid code
    mermaid_code = mmd_path.read_text(encoding='utf-8')
    print(f"📝 Mermaid code: {len(mermaid_code)} characters")
    print()
    
    # Try CLI first (better quality), fallback to API
    success = generate_svg_via_cli(mermaid_code, svg_path, mmd_path)
    
    if not success:
        print("\n🔄 Falling back to online API...")
        success = generate_svg_via_api(mermaid_code, svg_path)
    
    if success:
        print()
        print("=" * 60)
        print("🎉 SUCCESS! Architecture diagram generated.")
        print(f"📂 Location: {svg_path.relative_to(repo_root)}")
        print()
        print("📌 Usage:")
        print("  - Add to README: ![Architecture](zapabob/docs/codex-v0.48.0-architecture.svg)")
        print("  - Share on SNS: Upload the SVG file directly")
        print("  - View in browser: Open the SVG file")
        return 0
    else:
        print()
        print("=" * 60)
        print("❌ FAILED to generate SVG.")
        print("💡 Try installing Mermaid CLI:")
        print("   npm install -g @mermaid-js/mermaid-cli")
        return 1


if __name__ == "__main__":
    sys.exit(main())

