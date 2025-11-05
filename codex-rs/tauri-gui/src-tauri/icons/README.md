# Codex Icons

✅ **アイコン生成完了！**

このディレクトリには、Codex AI-Native OSのアプリケーションアイコンが含まれています。

## 📋 生成済みアイコン

- `32x32.png` - 32x32 pixel PNG
- `128x128.png` - 128x128 pixel PNG  
- `128x128@2x.png` - 256x256 pixel PNG (Retina)
- `icon.png` - 512x512 pixel PNG (タスクトレイ用)
- `icon.ico` - Windows ICO file (16/32/48/64/128/256px)
- `icon.icns.png` - macOS ICNS placeholder (512x512)
- `icon.iconset/` - macOS ICNS生成用ディレクトリ

## 🎨 ソースファイル

アイコンのソースは `../../.github/assets/codex-logo.svg` です。

## 🔄 再生成方法

アイコンを再生成する場合:

```bash
cd codex-tauri
node generate-icons.cjs
```

### 必要なパッケージ

```bash
npm install sharp png-to-ico
```

## 🍎 macOS ICNS生成 (macOSのみ)

```bash
cd src-tauri/icons
iconutil -c icns icon.iconset
mv icon.icns .
```

## 📦 使用箇所

- **デスクトップアイコン**: MSIインストール時に自動作成
- **タスクトレイアイコン**: `icon.png` (512x512)
- **ウィンドウアイコン**: `icon.ico` (Windows) / `icon.icns` (macOS)
- **スタートメニュー**: MSIインストール時に自動作成

## 🎯 アイコンデザイン

Codexのロゴは以下の要素で構成されています:

- 🔷 **ヘキサゴン**: AIの構造と安定性を表現
- ⚡ **コードブラケット `</>`**: コーディング支援を象徴
- 🧠 **ニューラルネットワーク**: AI/機械学習の要素
- 💙 **ブルーグラデーション**: 信頼性とテクノロジーを表現

---

**Generated**: 2025-11-03  
**Source**: `.github/assets/codex-logo.svg`  
**Generator**: `generate-icons.cjs`

