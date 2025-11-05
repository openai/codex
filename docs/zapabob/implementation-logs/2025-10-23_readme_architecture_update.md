# 2025-10-23 README.md アーキテクチャ図更新

## Summary
README.mdにMermaid形式のアーキテクチャ図とリポジトリ構造図を追加。SVG形式で出力完了。

## 追加内容

### 1. 詳細アーキテクチャ図（Mermaid）

**ファイル**: `zapabob/docs/codex-architecture-current.mmd`

**内容**:
- 🖥️ User Interface Layer（4コンポーネント）
- 🧠 Core Orchestration Layer（4コンポーネント）
- 🤖 Specialized Sub-Agents（8種類）
- 🔍 Deep Research Engine（5コンポーネント）
- 🔗 MCP Integration（14サーバー）

**特徴**:
- rmcp 0.8.3+ベストプラクティス明記
- Timeout: 5分、Retry: 3x
- Cache TTL: 1時間、45x高速化
- 動的エージェント選択
- メッセージパッシング（優先度0-255）

### 2. リポジトリ構造図（Mermaid）

**ファイル**: `zapabob/docs/repository-structure.mmd`

**内容**:
- 📦 Official OpenAI/codex（公式ディレクトリ）
- ⭐ zapabob Extensions（独自機能）
- ⚙️ Configuration（.cursor, .codex）
- 🗑️ Temporary（.gitignore対象）
- 📦 Archive（アーカイブ）

**構造**:
```
codex/
├── codex-rs/ (公式Rust実装)
├── zapabob/ (独自機能統一)
│   ├── docs/implementation-logs/ (236ファイル)
│   ├── scripts/
│   ├── extensions/
│   └── sdk/
├── _temp/ (.gitignore)
├── .cursor/ (Cursor設定)
└── .codex/ (Agent定義)
```

### 3. SVG出力

#### 生成ファイル
- `zapabob/docs/codex-architecture-current.svg` ✅
- `zapabob/docs/repository-structure.svg` ✅

#### 変換方法
kroki.io API使用:
1. Mermaidコンテンツ読み取り
2. zlib圧縮 + base64エンコード
3. https://kroki.io/mermaid/svg/{encoded}
4. SVGダウンロード

### 4. PNG出力（手動）

cairoライブラリの問題でPNG自動生成失敗。

**代替方法**:

#### 方法1: ブラウザで開いて保存
```powershell
# SVGをChromeで開く
start chrome zapabob/docs/codex-architecture-current.svg

# 右クリック → 名前を付けて画像を保存 → PNG形式
```

#### 方法2: ImageMagick（インストール必要）
```powershell
magick convert -density 300 zapabob/docs/codex-architecture-current.svg zapabob/docs/codex-architecture-current.png
```

#### 方法3: Inkscape（インストール必要）
```powershell
inkscape zapabob/docs/codex-architecture-current.svg --export-type=png --export-dpi=300
```

#### 方法4: オンラインツール
- https://cloudconvert.com/svg-to-png
- https://convertio.co/svg-png/

## README.md更新内容

### 追加セクション

#### 詳細アーキテクチャ図（折りたたみ）
```markdown
<details>
<summary>📊 <b>Detailed Architecture Diagram (Mermaid)</b></summary>

[Mermaid図のコード]

</details>
```

#### リポジトリ構造（折りたたみ）
```markdown
### 📁 Repository Structure

<details>
<summary><b>Directory Organization</b></summary>

[Mermaid図のコード]

</details>
```

### メリット

1. **視覚的理解**: アーキテクチャが一目で分かる
2. **GitHub対応**: GitHubがMermaidをレンダリング
3. **SVG形式**: 拡大縮小しても綺麗
4. **折りたたみ**: README.mdが長くならない
5. **保守性**: Mermaidコードで管理

## 生成スクリプト

### zapabob/scripts/mermaid-simple.py
```python
# Mermaid → SVG変換スクリプト
# kroki.io APIを使用
# 使用方法: python mermaid-simple.py input.mmd
```

**機能**:
- Mermaidファイル読み取り
- zlib圧縮 + base64エンコード
- kroki.io APIでSVG生成
- PNG変換試行（cairosvg）

## PNG生成手順（SNS用）

### 推奨: Chrome DevTools

1. **SVGを開く**
```powershell
start chrome zapabob/docs/codex-architecture-current.svg
```

2. **開発者ツールを開く**: F12

3. **デバイスツールバー**: Ctrl+Shift+M

4. **サイズ設定**: 2400 x 1800

5. **スクリーンショット**: Ctrl+Shift+P → "Capture screenshot"

6. **保存**: `zapabob/docs/codex-architecture-current.png`

### SNS最適サイズ

| SNS | 推奨サイズ | DPI |
|-----|----------|-----|
| Twitter/X | 1200 x 675 | 72 |
| LinkedIn | 1200 x 627 | 72 |
| GitHub Social | 1280 x 640 | 96 |
| Qiita | 1200 x 630 | 72 |

**生成方法**:
Chromeでサイズを調整してスクリーンショット

## 変更ファイル

### 新規作成
1. `zapabob/docs/codex-architecture-current.mmd` - アーキテクチャ図
2. `zapabob/docs/codex-architecture-current.svg` - SVG出力 ✅
3. `zapabob/docs/repository-structure.mmd` - リポジトリ構造図
4. `zapabob/docs/repository-structure.svg` - SVG出力 ✅
5. `zapabob/scripts/mermaid-simple.py` - 変換スクリプト
6. `zapabob/scripts/svg-to-png-browser.py` - PNG変換スクリプト
7. `zapabob/scripts/generate-mermaid-images.ps1` - PowerShellスクリプト
8. `_docs/2025-10-23_readme_architecture_update.md` - このログ

### 修正
1. `README.md` - アーキテクチャ図とリポジトリ構造追加

## 次のステップ

### 即時（手動）
- [ ] SVGをブラウザで開いてPNG保存
  - `codex-architecture-current.png`（SNS用: 1200x675）
  - `repository-structure.png`（SNS用: 1200x627）

### 自動化（将来）
- [ ] GitHub Actionsでアーキテクチャ図自動生成
- [ ] コミット時にMermaid→SVG変換
- [ ] PNG生成を自動化（ImageMagickまたはPuppeteer）

## Notes
- SVG生成成功（kroki.io API使用）
- PNG生成はcairoライブラリ依存で失敗
- 代替方法（Chrome DevTools）を提示
- README.mdにMermaid図埋め込み済み
- GitHubが自動レンダリング

**Status**: ✅ **SVG完了、PNG手動生成推奨**

