"""
Codex Icon Generator
SVGからTauriで必要な全アイコンフォーマットを生成
"""
import os
from pathlib import Path
from PIL import Image, ImageDraw
import cairosvg
import io

# パス設定
SCRIPT_DIR = Path(__file__).parent
SVG_PATH = SCRIPT_DIR.parent / ".github" / "assets" / "codex-logo.svg"
ICONS_DIR = SCRIPT_DIR / "src-tauri" / "icons"

# 必要なアイコンサイズ
ICON_SIZES = {
    "32x32.png": 32,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "icon.png": 512,  # タスクトレイ用
}

def svg_to_png(svg_path: Path, output_path: Path, size: int):
    """SVGをPNGに変換"""
    print(f"  📐 {size}x{size} -> {output_path.name}")
    
    # SVGをPNGに変換（cairosvg使用）
    png_data = cairosvg.svg2png(
        url=str(svg_path),
        output_width=size,
        output_height=size,
    )
    
    # PILで開いて保存（最適化）
    img = Image.open(io.BytesIO(png_data))
    img.save(output_path, "PNG", optimize=True)

def create_ico(base_sizes: list, output_path: Path):
    """複数サイズからWindows ICOファイルを作成"""
    print(f"  🪟 Windows ICO -> {output_path.name}")
    
    # 各サイズのPNG画像を生成
    images = []
    for size in base_sizes:
        png_data = cairosvg.svg2png(
            url=str(SVG_PATH),
            output_width=size,
            output_height=size,
        )
        img = Image.open(io.BytesIO(png_data))
        images.append(img)
    
    # ICOファイルとして保存
    images[0].save(
        output_path,
        format="ICO",
        sizes=[(img.width, img.height) for img in images],
        append_images=images[1:]
    )

def create_icns(base_sizes: list, output_path: Path):
    """複数サイズからmacOS ICNSファイルを作成"""
    print(f"  🍎 macOS ICNS -> {output_path.name}")
    
    # 一時的なiconsetディレクトリを作成
    iconset_dir = output_path.parent / "icon.iconset"
    iconset_dir.mkdir(exist_ok=True)
    
    # macOS ICNS用のサイズマッピング
    icns_sizes = {
        16: "icon_16x16.png",
        32: "icon_16x16@2x.png",
        32: "icon_32x32.png",
        64: "icon_32x32@2x.png",
        128: "icon_128x128.png",
        256: "icon_128x128@2x.png",
        256: "icon_256x256.png",
        512: "icon_256x256@2x.png",
        512: "icon_512x512.png",
        1024: "icon_512x512@2x.png",
    }
    
    # 各サイズのPNGを生成
    for size, filename in icns_sizes.items():
        png_data = cairosvg.svg2png(
            url=str(SVG_PATH),
            output_width=size,
            output_height=size,
        )
        img = Image.open(io.BytesIO(png_data))
        img.save(iconset_dir / filename, "PNG", optimize=True)
    
    # iconutilでICNSに変換（macOSのみ）
    import platform
    if platform.system() == "Darwin":
        import subprocess
        subprocess.run([
            "iconutil",
            "-c", "icns",
            str(iconset_dir),
            "-o", str(output_path)
        ])
    else:
        print("    ⚠️  macOS ICNS生成はmacOS環境でのみ利用可能")
        # Windowsでは簡易版（512x512のPNG）を作成
        png_data = cairosvg.svg2png(
            url=str(SVG_PATH),
            output_width=512,
            output_height=512,
        )
        img = Image.open(io.BytesIO(png_data))
        img.save(output_path.with_suffix('.png'), "PNG", optimize=True)
        print(f"    ℹ️  代わりにicon.icns.pngを作成（後でmacOSで変換してください）")
    
    # 一時ディレクトリを削除
    import shutil
    if iconset_dir.exists():
        shutil.rmtree(iconset_dir)

def main():
    """メイン処理"""
    print("🎨 Codex Icon Generator")
    print("=" * 50)
    
    # SVGファイルの存在確認
    if not SVG_PATH.exists():
        print(f"❌ SVGファイルが見つかりません: {SVG_PATH}")
        return
    
    print(f"📂 入力: {SVG_PATH}")
    print(f"📂 出力: {ICONS_DIR}")
    print()
    
    # iconsディレクトリを作成
    ICONS_DIR.mkdir(parents=True, exist_ok=True)
    
    # PNG画像を生成
    print("🖼️  PNG画像を生成中...")
    for filename, size in ICON_SIZES.items():
        output_path = ICONS_DIR / filename
        svg_to_png(SVG_PATH, output_path, size)
    
    print()
    
    # Windows ICOを生成
    print("🖼️  プラットフォーム固有アイコンを生成中...")
    ico_sizes = [16, 32, 48, 64, 128, 256]
    create_ico(ico_sizes, ICONS_DIR / "icon.ico")
    
    # macOS ICNSを生成
    create_icns([16, 32, 128, 256, 512, 1024], ICONS_DIR / "icon.icns")
    
    print()
    print("✅ アイコン生成完了！")
    print()
    print("📋 生成されたファイル:")
    for file in sorted(ICONS_DIR.glob("*")):
        if file.suffix in ['.png', '.ico', '.icns']:
            size_kb = file.stat().st_size / 1024
            print(f"  ✓ {file.name} ({size_kb:.1f} KB)")
    
    print()
    print("🚀 次のステップ:")
    print("  1. codex-tauri/src-tauri/tauri.conf.json を確認")
    print("  2. タスクトレイアイコンの動作を確認")
    print("  3. アプリケーションをビルド: npm run tauri build")

if __name__ == "__main__":
    # 必要なライブラリチェック
    try:
        import cairosvg
        from PIL import Image
    except ImportError as e:
        print("❌ 必要なライブラリがインストールされていません")
        print()
        print("以下のコマンドを実行してください:")
        print("  py -3 -m pip install pillow cairosvg")
        print()
        import sys
        sys.exit(1)
    
    main()

