# 🚀 Codex MCP デモ

**日時**: 2025-10-12  
**バージョン**: codex-cli 0.47.0-alpha.1

---

## ✅ 登録済みMCPサーバー

現在、以下の3つのMCPサーバーが登録されています：

### 1. **codex-agent** 
- **コマンド**: `codex mcp-server`
- **機能**: 自己参照型Codex（メタオーケストレーション）
- **用途**: Codex が自分自身をツールとして使用

### 2. **playwright**
- **コマンド**: `npx -y @playwright/mcp`
- **機能**: ブラウザ自動操作
- **用途**: 
  - E2Eテスト生成
  - スクリーンショット取得
  - ブラウザ自動化

### 3. **web-search**
- **コマンド**: `npx -y @modelcontextprotocol/server-brave-search`
- **機能**: Web検索（Brave Search）
- **用途**: 
  - リアルタイム情報検索
  - 技術ドキュメント検索
  - 最新情報の取得

---

## 🛠️ 基本的な使い方

### MCPサーバー一覧を確認
```bash
codex mcp list
```

### 新しいMCPサーバーを追加
```bash
codex mcp add <name> -- <command> <args...>
```

**例**:
```bash
# Playwright追加
codex mcp add playwright -- npx -y @playwright/mcp

# Web検索追加
codex mcp add web-search -- npx -y @modelcontextprotocol/server-brave-search
```

### MCPサーバーを削除
```bash
codex mcp remove <name>
```

---

## 🎯 実践例

### 例1: Web検索を使った情報収集

**プロンプト**:
```
Use the web-search MCP server to find the latest information about Rust async/await best practices
```

**期待される動作**:
- Brave Search API を使用
- 最新のRust情報を検索
- ベストプラクティスをまとめる

---

### 例2: Playwrightでスクリーンショット取得

**プロンプト**:
```
Use the playwright MCP server to take a screenshot of https://www.rust-lang.org/
```

**期待される動作**:
- ブラウザを自動起動
- 指定URLにアクセス
- スクリーンショットを保存

---

### 例3: 自己参照型オーケストレーション

**プロンプト**:
```
Use the codex-agent MCP server to create a new sub-agent that specializes in security auditing, then have it review the authentication module
```

**期待される動作**:
- Codex が別のCodexインスタンスを起動
- セキュリティ専門のサブエージェントを生成
- 認証モジュールのレビューを実行

---

## 📊 MCPサーバーのアーキテクチャ

```
┌─────────────────────────────────────────────┐
│           Codex CLI (Main Process)          │
├─────────────────────────────────────────────┤
│  - Prompt processing                        │
│  - LLM communication (OpenAI API)           │
│  - Tool orchestration                       │
└───────────────┬─────────────────────────────┘
                │
                │ JSON-RPC (stdio)
                │
    ┌───────────┴───────────┐
    │                       │
    ▼                       ▼
┌─────────┐          ┌─────────────┐
│ codex-  │          │ playwright  │
│ agent   │          │             │
└─────────┘          └─────────────┘
    │                       │
    │                       ▼
    │                ┌─────────────────┐
    │                │ Browser Control │
    │                └─────────────────┘
    ▼
┌─────────────┐
│ web-search  │
└─────────────┘
    │
    ▼
┌─────────────────┐
│ Brave Search API│
└─────────────────┘
```

---

## 🔐 セキュリティ設定

MCPサーバーは `~/.codex/config.toml` のサンドボックス設定に従います。

**推奨設定**:
```toml
[sandbox]
mode = "workspace-write"  # ワークスペース内のみ書き込み可能

[mcp_servers.web-search]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-brave-search"]
env.BRAVE_API_KEY = "your-api-key-here"

[mcp_servers.playwright]
command = "npx"
args = ["-y", "@playwright/mcp"]
```

---

## 🎉 メリット

### 1. **拡張性**
- 任意の外部ツールを統合可能
- npm パッケージを簡単に追加

### 2. **標準化**
- JSON-RPC による統一的な通信
- 互換性の高いプロトコル

### 3. **自然言語操作**
- CLIコマンドを覚える必要なし
- プロンプトで直接指示

### 4. **メタオーケストレーション**
- Codex が Codex を呼び出せる
- 無限にスケーラブル

---

## 📚 参考リソース

### 公式ドキュメント
- [Codex Documentation](https://github.com/zapabob/codex)
- [MCP Specification](https://modelcontextprotocol.io/)

### 追加できる人気MCPサーバー
1. **@modelcontextprotocol/server-filesystem** - ファイル操作
2. **@modelcontextprotocol/server-github** - GitHub API
3. **@modelcontextprotocol/server-google-maps** - Google Maps
4. **@modelcontextprotocol/server-postgres** - PostgreSQL操作
5. **@modelcontextprotocol/server-puppeteer** - ブラウザ自動化
6. **@modelcontextprotocol/server-slack** - Slack統合
7. **@modelcontextprotocol/server-fetch** - HTTP リクエスト

---

## 🚀 次のステップ

### ステップ1: APIキーの設定
```bash
# Brave Search API キーを取得
# https://brave.com/search/api/

# 環境変数に設定
export BRAVE_API_KEY="your-key-here"
```

### ステップ2: 実際に使ってみる
```bash
codex "Use web-search to find the top 5 Rust crates for async programming"
```

### ステップ3: カスタムMCPサーバーを作成
```typescript
// custom-mcp-server.ts
import { Server } from '@modelcontextprotocol/sdk';

const server = new Server({
  name: 'my-custom-server',
  version: '1.0.0',
});

server.tool('my-tool', async (params) => {
  // Custom logic
  return { result: 'Hello from custom MCP!' };
});

server.start();
```

---

## 🎯 まとめ

Codex MCP を使うことで、CLI から様々な外部ツールを自然言語で操作できるようになります。

**主な特徴**:
- ✅ 3つの強力なMCPサーバーを登録済み
- ✅ 自然言語での操作が可能
- ✅ 無限に拡張可能
- ✅ 標準化されたプロトコル
- ✅ メタオーケストレーション対応

**次のアクション**: 実際に使ってみて、開発効率を体感してください！🚀

---

**作成者**: zapabob  
**作成日**: 2025-10-12  
**Codex Version**: 0.47.0-alpha.1

