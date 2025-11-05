/**
 * Codex Icon Generator (Node.js版)
 * SVGからTauriで必要な全アイコンフォーマットを生成
 */
const sharp = require('sharp');
const fs = require('fs').promises;
const path = require('path');
const { createCanvas, loadImage } = require('canvas');
const ico = require('sharp-ico');

// パス設定
const SVG_PATH = path.join(__dirname, '..', '.github', 'assets', 'codex-logo.svg');
const ICONS_DIR = path.join(__dirname, 'src-tauri', 'icons');

// 必要なアイコンサイズ
const ICON_SIZES = {
  '32x32.png': 32,
  '128x128.png': 128,
  '128x128@2x.png': 256,
  'icon.png': 512,  // タスクトレイ用
};

// Windows ICO用のサイズ
const ICO_SIZES = [16, 32, 48, 64, 128, 256];

// macOS ICNS用のサイズマッピング
const ICNS_SIZES = {
  16: 'icon_16x16.png',
  32: 'icon_16x16@2x.png',
  128: 'icon_128x128.png',
  256: 'icon_128x128@2x.png',
  512: 'icon_512x512.png',
  1024: 'icon_512x512@2x.png',
};

/**
 * SVGをPNGに変換
 */
async function svgToPng(svgPath, outputPath, size) {
  console.log(`  📐 ${size}x${size} -> ${path.basename(outputPath)}`);
  
  await sharp(svgPath)
    .resize(size, size, {
      fit: 'contain',
      background: { r: 0, g: 0, b: 0, alpha: 0 }
    })
    .png()
    .toFile(outputPath);
}

/**
 * Windows ICOファイルを作成
 */
async function createIco(svgPath, outputPath) {
  console.log(`  🪟 Windows ICO -> ${path.basename(outputPath)}`);
  
  // 各サイズのPNGバッファを生成
  const buffers = await Promise.all(
    ICO_SIZES.map(size =>
      sharp(svgPath)
        .resize(size, size, {
          fit: 'contain',
          background: { r: 0, g: 0, b: 0, alpha: 0 }
        })
        .png()
        .toBuffer()
    )
  );
  
  // ICOファイルとして保存
  const icoBuffer = await ico.encode(buffers.map((buf, i) => ({
    data: buf,
    width: ICO_SIZES[i],
    height: ICO_SIZES[i]
  })));
  
  await fs.writeFile(outputPath, icoBuffer);
}

/**
 * macOS ICNS用のPNG画像を生成（iconutilは後で手動実行）
 */
async function createIcnsImages(svgPath, iconsDir) {
  console.log(`  🍎 macOS ICNS images -> icon.iconset/`);
  
  // iconsetディレクトリを作成
  const iconsetDir = path.join(iconsDir, 'icon.iconset');
  await fs.mkdir(iconsetDir, { recursive: true });
  
  // 各サイズのPNGを生成
  for (const [size, filename] of Object.entries(ICNS_SIZES)) {
    const outputPath = path.join(iconsetDir, filename);
    await sharp(svgPath)
      .resize(parseInt(size), parseInt(size), {
        fit: 'contain',
        background: { r: 0, g: 0, b: 0, alpha: 0 }
      })
      .png()
      .toFile(outputPath);
  }
  
  console.log(`    ℹ️  iconsetディレクトリを作成しました`);
  console.log(`    📝 macOSで以下を実行: iconutil -c icns icon.iconset`);
  
  // プレースホルダーとして512x512のPNGを作成
  const placeholderPath = path.join(iconsDir, 'icon.icns.png');
  await sharp(svgPath)
    .resize(512, 512, {
      fit: 'contain',
      background: { r: 0, g: 0, b: 0, alpha: 0 }
    })
    .png()
    .toFile(placeholderPath);
}

/**
 * メイン処理
 */
async function main() {
  console.log('🎨 Codex Icon Generator');
  console.log('='.repeat(50));
  
  // SVGファイルの存在確認
  try {
    await fs.access(SVG_PATH);
  } catch (error) {
    console.error(`❌ SVGファイルが見つかりません: ${SVG_PATH}`);
    process.exit(1);
  }
  
  console.log(`📂 入力: ${SVG_PATH}`);
  console.log(`📂 出力: ${ICONS_DIR}`);
  console.log();
  
  // iconsディレクトリを作成
  await fs.mkdir(ICONS_DIR, { recursive: true });
  
  // PNG画像を生成
  console.log('🖼️  PNG画像を生成中...');
  for (const [filename, size] of Object.entries(ICON_SIZES)) {
    const outputPath = path.join(ICONS_DIR, filename);
    await svgToPng(SVG_PATH, outputPath, size);
  }
  
  console.log();
  
  // Windows ICOを生成
  console.log('🖼️  プラットフォーム固有アイコンを生成中...');
  const icoPath = path.join(ICONS_DIR, 'icon.ico');
  await createIco(SVG_PATH, icoPath);
  
  // macOS ICNS用の画像を生成
  await createIcnsImages(SVG_PATH, ICONS_DIR);
  
  console.log();
  console.log('✅ アイコン生成完了！');
  console.log();
  console.log('📋 生成されたファイル:');
  
  const files = await fs.readdir(ICONS_DIR);
  for (const file of files.sort()) {
    if (['.png', '.ico'].some(ext => file.endsWith(ext))) {
      const filePath = path.join(ICONS_DIR, file);
      const stats = await fs.stat(filePath);
      const sizeKb = (stats.size / 1024).toFixed(1);
      console.log(`  ✓ ${file} (${sizeKb} KB)`);
    }
  }
  
  console.log();
  console.log('🚀 次のステップ:');
  console.log('  1. codex-tauri/src-tauri/tauri.conf.json を確認');
  console.log('  2. タスクトレイアイコンの動作を確認');
  console.log('  3. アプリケーションをビルド: npm run tauri build');
  console.log();
  console.log('🍎 macOS ICNS生成 (macOSのみ):');
  console.log('  cd src-tauri/icons');
  console.log('  iconutil -c icns icon.iconset');
  console.log('  mv icon.icns .');
}

// 実行
main().catch(error => {
  console.error('❌ エラーが発生しました:', error);
  console.error();
  console.error('必要なパッケージをインストール:');
  console.error('  npm install sharp sharp-ico canvas');
  process.exit(1);
});

