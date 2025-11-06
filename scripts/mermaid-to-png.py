#!/usr/bin/env python3
"""
Mermaid diagram to PNG converter using mmdc CLI
"""
import subprocess
import sys
from pathlib import Path

def convert_mermaid_to_png(input_file, output_file, width=1200, height=630):
    """Convert mermaid file to PNG using mmdc"""
    cmd = [
        "mmdc",
        "-i", str(input_file),
        "-o", str(output_file),
        "-w", str(width),
        "-H", str(height),
        "-b", "white",
    ]
    
    print(f"🎨 Converting {input_file} → {output_file} ({width}x{height})")
    
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        print(f"✅ 成功: {output_file}")
        return True
    except subprocess.CalledProcessError as e:
        print(f"❌ 失敗: {e.stderr}")
        return False

def main():
    base_dir = Path(__file__).parent.parent
    input_file = base_dir / "architecture-v2.0.0.mmd"
    
    if not input_file.exists():
        print(f"❌ ファイルが見つかりません: {input_file}")
        sys.exit(1)
    
    # Read and validate mermaid file
    content = input_file.read_text(encoding='utf-8')
    if not content.startswith('graph'):
        print(f"⚠️  ファイルがgraphで始まっていません。修正中...")
        # Fix BOM issue
        content = content.lstrip('\ufeff')
        input_file.write_text(content, encoding='utf-8')
    
    print(f"📊 Mermaid ファイル: {len(content)} characters")
    
    # Convert to different sizes
    conversions = [
        ("architecture-v2.0.0-twitter.png", 1200, 630, "X/Twitter"),
        ("architecture-v2.0.0-linkedin.png", 1200, 627, "LinkedIn"),
        ("architecture-v2.0.0.png", 2400, 1800, "Generic"),
    ]
    
    success_count = 0
    for filename, width, height, platform in conversions:
        output_file = base_dir / filename
        if convert_mermaid_to_png(input_file, output_file, width, height):
            success_count += 1
            file_size = output_file.stat().st_size / 1024
            print(f"  ✅ {platform}: {file_size:.2f} KB")
    
    print(f"\n📊 変換結果: {success_count}/{len(conversions)} 成功")
    
    if success_count == 0:
        print("\n⚠️  mmdファイルを手動確認してください")
        sys.exit(1)

if __name__ == "__main__":
    main()

