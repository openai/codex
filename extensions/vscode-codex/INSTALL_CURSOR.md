# Cursor統合用VSIXインストールガイド

**Version**: 0.57.0  
**最終更新**: 2025-11-09

---

## 🎯 概要

このガイドでは、Codex VSIX拡張機能をCursor IDEにインストールして統合する方法を説明するで〜

---

## 📦 前提条件

### 1. Codex CLIのインストール

```powershell
# Rustプロジェクトからビルド＆インストール
cd codex-rs
cargo build --release -p codex-cli
cargo install --path cli --force

# 動作確認
codex --version
# => codex-cli 0.57.0 またはそれ以降
```

### 2. Cursor IDEのインストール

- [Cursor公式サイト](https://cursor.sh/)からダウンロード
- Windows版をインストール済みであること

---

## 🚀 VSIXパッケージの作成

### 方法1: 自動パッケージングスクリプト（推奨）

```powershell
cd extensions/vscode-codex
.\package-vsix.ps1 -Version 0.57.0 -Clean -Install
```

**オプション**:
- `-Version`: バージョン指定（省略時はpackage.jsonから自動取得）
- `-Clean`: クリーンビルド実行
- `-Install`: パッケージング後に自動インストール

### 方法2: 手動パッケージング

```powershell
cd extensions/vscode-codex

# 依存関係インストール
npm install

# TypeScriptコンパイル
npm run compile

# VSIXパッケージ作成
npm run package
# => codex-assistant-0.57.0.vsix が生成される
```

---

## 📥 VSIXインストール

### 方法1: コマンドラインからインストール

```powershell
# Cursorのパスを指定してインストール
$cursorPath = "$env:LOCALAPPDATA\Programs\cursor\Cursor.exe"
& $cursorPath --install-extension extensions/vscode-codex/codex-assistant-0.57.0.vsix
```

### 方法2: Cursor UIからインストール

1. Cursorを起動
2. `Ctrl+Shift+X` で拡張機能パネルを開く
3. `...` メニューから「VSIXからインストール...」を選択
4. `codex-assistant-0.57.0.vsix` を選択

### 方法3: ドラッグ&ドロップ

1. Cursorを起動
2. `codex-assistant-0.57.0.vsix` をCursorウィンドウにドラッグ&ドロップ

---

## ⚙️ Cursor統合設定

### 自動設定（推奨）

拡張機能をインストールすると、Cursor環境を自動検出してMCP設定ファイルを生成するで〜

**生成される設定ファイル**: `.cursor/mcp.json`

```json
{
  "mcpServers": {
    "codex": {
      "command": "codex",
      "args": ["mcp-server"],
      "env": {},
      "description": "Codex Multi-Agent System with Deep Research, Sub-Agents, and Blueprint Mode",
      "disabled": false
    }
  }
}
```

### 手動設定

もし自動設定が動作しない場合は、手動で設定してくれ：

1. **コマンドパレット** (`Ctrl+Shift+P`) を開く
2. `Codex: Generate MCP Config for Cursor` を実行
3. `.cursor/mcp.json` が生成される

### 設定ファイルの確認

- **コマンドパレット**: `Codex: Open MCP Config File`
- **ファイルパス**: ワークスペースルートの `.cursor/mcp.json`

---

## 🔄 Cursor再起動

MCP設定を変更した場合は、**Cursorを再起動**する必要があるで〜

```powershell
# Cursorを完全に終了
Get-Process cursor | Stop-Process -Force

# 再起動
Start-Process "$env:LOCALAPPDATA\Programs\cursor\Cursor.exe"
```

---

## ✅ 動作確認

### 1. 拡張機能の確認

1. Cursorを起動
2. `Ctrl+Shift+X` で拡張機能パネルを開く
3. `Codex AI Assistant` がインストールされていることを確認

### 2. Orchestratorの起動確認

1. **コマンドパレット** (`Ctrl+Shift+P`) を開く
2. `Codex: Start Orchestrator` を実行
3. ステータスバーに「🟢 Running」が表示されることを確認

### 3. MCPサーバーの確認

1. **サイドバー**の「Codex AI」パネルを開く
2. 「MCP Servers」ビューで `codex` が表示されることを確認

### 4. Cursor Composerでの使用

1. **Composer** (`Ctrl+I`) を開く
2. `@codex` を入力してCodexエージェントを呼び出し
3. タスクを実行して動作確認

---

## 🛠️ トラブルシューティング

### VSIXインストールが失敗する

**原因**: Cursorのバージョンが古い可能性

**解決策**:
```powershell
# Cursorを最新版に更新
# または、VS Code互換モードでインストール
code --install-extension codex-assistant-0.57.0.vsix
```

### Orchestratorが起動しない

**原因**: Codex CLIがインストールされていない、またはPATHに含まれていない

**解決策**:
```powershell
# Codex CLIのインストール確認
codex --version

# PATHに含まれているか確認
where.exe codex

# 含まれていない場合は、インストールパスをPATHに追加
$env:PATH += ";C:\Users\$env:USERNAME\.cargo\bin"
```

### MCP設定が生成されない

**原因**: Cursor環境が検出されていない

**解決策**:
1. **コマンドパレット**で `Codex: Generate MCP Config for Cursor` を手動実行
2. ワークスペースルートに `.cursor/mcp.json` が生成されることを確認

### MCPサーバーに接続できない

**原因**: Codex MCPサーバーが起動していない

**解決策**:
```powershell
# MCPサーバーを手動起動してテスト
codex mcp-server

# 別のターミナルで接続テスト
codex orchestrator status
```

---

## 📚 関連ドキュメント

- [Codex README](../../README.md)
- [Cursor統合ガイド](../../docs/guides/cursor-integration.md)
- [MCPサーバー情報](../../codex-rs/mcp-server-info.md)

---

## 🎉 完了！

これでCursorとCodexが完全に統合されたで〜！

**次のステップ**:
- `@codex` でComposerからCodexを呼び出し
- `Ctrl+Shift+D` でタスクをエージェントに委譲
- `Ctrl+Shift+R` で深層研究を実行

**Happy Coding! 🚀**

