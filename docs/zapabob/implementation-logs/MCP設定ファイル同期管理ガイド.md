# MCP設定ファイル同期管理ガイド

**バージョン**: 0.48.0-zapabob.1  
**作成日**: 2025-10-23  

## 📋 概要

CodexとCursor IDEは異なるMCP設定ファイルを使用します：

| ツール | 設定ファイル | フォーマット |
|--------|------------|------------|
| **Codex CLI** | `config.toml` | TOML |
| **Cursor IDE** | `c:\Users\downl\.cursor\mcp.json` | JSON |

両ファイルを同期して管理する必要があります。

## 🔧 設定ファイル構造

### config.toml (Codex CLI用)

```toml
[mcp_servers.サーバー名]
command = "コマンド"
args = ["引数1", "引数2"]
env.ENV_VAR = "値"
description = "説明"
```

### mcp.json (Cursor IDE用)

```json
{
  "mcpServers": {
    "サーバー名": {
      "type": "stdio",
      "command": "コマンド",
      "args": ["引数1", "引数2"],
      "env": {
        "ENV_VAR": "値"
      },
      "description": "説明",
      "disabled": false
    }
  }
}
```

## 📊 現在の設定状況

### 共通MCPサーバー（両ファイルに存在）

1. **codex** - Codexメインサーバー
2. **serena** - AIオーケストレーション
3. **context7** - Upstashコンテキスト管理
4. **playwright** - Web自動化
5. **filesystem** - ファイルシステム操作
6. **github** - GitHub API操作
7. **markitdown** - Markdown変換
8. **arxiv-mcp-server** - arXiv論文検索
9. **youtube** - YouTube動画操作
10. **chrome-devtools** - Chrome DevTools
11. **codex-gemini-mcp** - Gemini CLI MCP統合 ✨

### Codex CLI専用（config.tomlのみ）

12. **codex-supervisor** - マルチエージェント調整
13. **codex-research** - ディープリサーチ
14. **codex-agent** - 自然言語CLI

## 🔄 新規MCPサーバー追加手順

### 1. config.tomlに追加

```toml
[mcp_servers.新サーバー名]
command = "コマンド"
args = ["引数"]
env.変数名 = "値"
description = "説明"
```

### 2. mcp.jsonに追加

```json
{
  "mcpServers": {
    "新サーバー名": {
      "type": "stdio",
      "command": "コマンド",
      "args": ["引数"],
      "env": {
        "変数名": "値"
      },
      "description": "説明",
      "disabled": false
    }
  }
}
```

### 3. Cursor IDEを再起動

設定を反映させるためにCursor IDEを再起動します。

## ✅ 変換ルール

| config.toml | mcp.json |
|------------|----------|
| `[mcp_servers.名前]` | `"mcpServers": { "名前": { ... } }` |
| `command = "cmd"` | `"command": "cmd"` |
| `args = ["a"]` | `"args": ["a"]` |
| `env.VAR = "val"` | `"env": { "VAR": "val" }` |
| `description = "..."` | `"description": "..."` |
| （不要） | `"type": "stdio"` |
| （不要） | `"disabled": false` |

## 🎯 重要な注意点

### 1. パス表記
- **config.toml**: Windowsパスは `\\` でエスケープ
  ```toml
  env.PATH = "C:\\Users\\downl\\.cargo\\bin;${PATH}"
  ```
- **mcp.json**: Windowsパスは `\\` でエスケープ
  ```json
  "PATH": "C:\\Users\\downl\\.cargo\\bin;${PATH}"
  ```

### 2. 環境変数参照
- 両ファイルとも `${変数名}` 形式で参照可能

### 3. 配列とオブジェクト
- **TOML**: `args = ["a", "b"]`
- **JSON**: `"args": ["a", "b"]`

### 4. コメント
- **TOML**: `#` でコメント可能
- **JSON**: コメント不可（`description`フィールドを活用）

## 🔍 検証コマンド

### config.toml検証
```powershell
Select-String -Path "config.toml" -Pattern "\[mcp_servers\."
```

### mcp.json検証
```powershell
Get-Content "c:\Users\downl\.cursor\mcp.json" | ConvertFrom-Json | Select-Object -ExpandProperty mcpServers | Get-Member -MemberType NoteProperty
```

## 📝 最新追加: codex-gemini-mcp

### config.toml
```toml
[mcp_servers.codex-gemini-mcp]
args = []
command = "codex-gemini-mcp"
env.PATH = "C:\\Users\\downl\\.cargo\\bin;${PATH}"
description = "Codex Gemini CLI MCP Server v0.48.0 - Google Gemini AI integration with OAuth 2.0 authentication and Google Search Grounding"
```

### mcp.json
```json
"codex-gemini-mcp": {
  "type": "stdio",
  "command": "codex-gemini-mcp",
  "args": [],
  "env": {
    "PATH": "C:\\Users\\downl\\.cargo\\bin;${PATH}"
  },
  "description": "Codex Gemini CLI MCP Server v0.48.0 - Google Gemini AI integration with OAuth 2.0 authentication and Google Search Grounding",
  "disabled": false
}
```

## 🚀 使用例

### Codex CLIから
```bash
codex research "query" --gemini --use-mcp
```

### Cursor IDEから
- MCPサーバー一覧で`codex-gemini-mcp`を確認
- `@codex-gemini-mcp googleSearch`でツール呼び出し

## 🛠️ トラブルシューティング

### 設定が反映されない
1. Cursor IDEを再起動
2. `codex --version`でバージョン確認
3. MCPサーバーのパスが正しいか確認

### エラーが出る
1. コマンドがPATHに含まれているか確認
2. 環境変数が正しく設定されているか確認
3. バイナリが存在するか確認

---
**更新日**: 2025-10-23  
**ステータス**: ✅ 統合完了

