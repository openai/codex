# 2025-10-25 MCP サーバー診断結果

## 📊 実行サマリー

**実行日時**: 2025-10-25  
**テスト環境**: Windows 11, Codex v0.49.0-zapabob.1  
**目的**: 全MCPサーバーの動作確認とトラブルシューティング

---

## ✅ 動作成功サーバー（2/15）

### 1. codex (mcp-server)
**ステータス**: ✅ 正常動作  
**起動時間**: < 1秒  
**利用可能ツール数**: 7個

**ツール一覧**:
1. `codex` - Codex coding session起動
2. `codex-reply` - 既存Codex会話の継続
3. `codex-subagent` - Codexサブエージェント管理
4. `codex-supervisor` - マルチエージェント調整
5. `codex-auto-orchestrate` - タスク分析&エージェントオーケストレーション
6. `codex-deep-research` - 構造化マルチソース調査
7. `codex-custom-command` - 定義済みサブエージェントコマンド実行
8. `codex-hook` - ライフサイクルフック実行

**設定**:
```toml
[mcp_servers.codex]
args = ["mcp-server"]
command = "codex"
startup_timeout_sec = 60
env.GITHUB_TOKEN = "${GITHUB_TOKEN}"
env.OPENAI_API_KEY = "${OPENAI_API_KEY}"
env.RUST_LOG = "info"
env.SLACK_WEBHOOK_URL = "${SLACK_WEBHOOK_URL}"
```

---

### 2. codex-gemini-mcp
**ステータス**: ✅ 正常動作  
**起動時間**: < 1秒  
**利用可能ツール数**: 1個

**ツール一覧**:
1. `googleSearch` - Google検索（Gemini Grounding経由、OAuth 2.0認証）

**設定**:
```toml
[mcp_servers.codex-gemini-mcp]
args = []
command = "codex-gemini-mcp"
description = "Codex Gemini CLI MCP Server v0.49.0"
env.PATH = "C:\\Users\\downl\\.cargo\\bin;${PATH}"
startup_timeout_sec = 60
```

---

## ❌ 起動失敗サーバー（13/15）

### タイムアウトエラー（60秒）

#### 1. brave-search
**エラー**: `timed out handshaking with MCP server after 60s`  
**原因推定**: 
- npxパッケージのダウンロード遅延
- BRAVE_API_KEY環境変数未設定
- ネットワーク接続問題

**設定**:
```toml
[mcp_servers.brave-search]
args = ["-y", "@modelcontextprotocol/server-brave-search"]
command = "C:\\nvm4w\\nodejs\\npx.cmd"
startup_timeout_sec = 60
disabled = true
env.BRAVE_API_KEY = "${BRAVE_API_KEY}"
```

#### 2. playwright
**エラー**: `timed out handshaking with MCP server after 60s`  
**原因推定**: 
- Playwright初回インストールに時間がかかる
- ブラウザバイナリのダウンロード待ち

#### 3. context7
**エラー**: `timed out handshaking with MCP server after 60s`  
**原因推定**: 
- Upstash API接続タイムアウト
- 認証情報未設定

#### 4. filesystem
**エラー**: `timed out handshaking with MCP server after 60s`  
**原因推定**: 
- npxパッケージの初回ダウンロード遅延

#### 5. github
**エラー**: `timed out handshaking with MCP server after 60s`  
**原因推定**: 
- GITHUB_TOKEN環境変数未設定または無効

#### 6. youtube
**エラー**: `timed out handshaking with MCP server after 60s`  
**原因推定**: 
- npmパッケージの依存関係問題

#### 7. chrome-devtools
**エラー**: `timed out handshaking with MCP server after 60s`  
**原因推定**: 
- Chrome DevTools Protocolの接続問題

---

### 接続クローズエラー（initialize response）

#### 8. serena
**エラー**: `handshaking with MCP server failed: connection closed: initialize response`  
**原因推定**: 
- uvx経由でのGitリポジトリクローンに失敗
- Pythonパッケージの依存関係エラー
- serena自体の初期化エラー

**設定**:
```toml
[mcp_servers.serena]
args = [
    "--from",
    "git+https://github.com/oraios/serena",
    "serena",
    "start-mcp-server",
    "--context",
    "codex",
]
command = "C:\\Users\\downl\\.local\\bin\\uvx.exe"
startup_timeout_sec = 90
```

#### 9. markitdown
**エラー**: `handshaking with MCP server failed: connection closed: initialize response`  
**原因推定**: 
- uvx経由でのパッケージインストール失敗
- Pythonバージョン互換性問題

**設定**:
```toml
[mcp_servers.markitdown]
args = ["markitdown-mcp"]
command = "C:\\Users\\downl\\.local\\bin\\uvx.exe"
startup_timeout_sec = 60
```

#### 10. arxiv-mcp-server
**エラー**: `handshaking with MCP server failed: connection closed: initialize response`  
**原因推定**: 
- uvx経由でのパッケージインストール失敗
- arxiv APIへの接続問題

**設定**:
```toml
[mcp_servers.arxiv-mcp-server]
args = ["arxiv-mcp-server"]
command = "C:\\Users\\downl\\.local\\bin\\uvx.exe"
startup_timeout_sec = 60
```

---

### 未実装サブコマンド

#### 11. codex-supervisor
**エラー**: `handshaking with MCP server failed: connection closed: initialize response`  
**原因**: `codex supervisor` サブコマンドが未実装  
**確認方法**: `codex --help` でサブコマンド一覧を確認

**推奨設定**:
```toml
[mcp_servers.codex-supervisor]
# ⚠️ Note: 'supervisor' subcommand not implemented yet
disabled = true
```

#### 12. codex-research
**エラー**: `handshaking with MCP server failed: connection closed: initialize response`  
**原因**: `codex research` サブコマンドが未実装  
**確認方法**: `codex --help` でサブコマンド一覧を確認

**推奨設定**:
```toml
[mcp_servers.codex-research]
# ⚠️ Note: 'research' subcommand not implemented yet
disabled = true
```

#### 13. codex-agent
**エラー**: `handshaking with MCP server failed: connection closed: initialize response`  
**原因**: `codex agent` サブコマンドが未実装（`codex agent-create` は存在）  
**確認方法**: `codex --help` でサブコマンド一覧を確認

**推奨設定**:
```toml
[mcp_servers.codex-agent]
# ⚠️ Note: 'agent' subcommand not implemented yet (use 'agent-create' instead)
disabled = true
```

---

## 🔍 根本原因分析

### 1. disabled = true が機能しない

**問題**: config.tomlで `disabled = true` を設定しても、MCPサーバーの起動を試みる

**原因**: 
- Codex CLIの `experimental_use_rmcp_client = true` モードでは、`disabled` フラグが正しく処理されていない可能性
- または、`codex exec` コマンドが全サーバーの起動を試みる設計

**影響**: 
- 起動に失敗するサーバーのせいで、全体の起動時間が60秒×失敗サーバー数分延長される
- 今回の場合: 13サーバー × 60秒 = 最大13分の遅延

**回避策**:
1. config.tomlから不要なサーバーを完全に削除
2. 別の設定ファイル（minimal-config.toml）を作成して使用
3. 環境変数で無効化（未確認）

---

### 2. uvx系サーバーの接続クローズ

**問題**: uvx経由のPythonパッケージが初期化段階で失敗

**原因推定**:
- uvx自体の問題（バージョン不整合）
- Pythonパッケージの依存関係エラー
- MCPプロトコルの実装不備

**影響サーバー**: serena, markitdown, arxiv-mcp-server

**デバッグ方法**:
```powershell
# 手動起動テスト
uvx markitdown-mcp
uvx arxiv-mcp-server
uvx --from git+https://github.com/oraios/serena serena start-mcp-server --context codex
```

---

### 3. npx系サーバーのタイムアウト

**問題**: npx経由のNode.jsパッケージが60秒以内に応答しない

**原因推定**:
- 初回起動時のパッケージダウンロード遅延
- ネットワーク帯域幅制限
- パッケージの依存関係解決に時間がかかる

**影響サーバー**: brave-search, playwright, context7, filesystem, github, youtube, chrome-devtools

**デバッグ方法**:
```powershell
# 手動起動テスト（初回ダウンロード実施）
npx -y @modelcontextprotocol/server-brave-search
npx -y @playwright/mcp@latest
npx -y @upstash/context7-mcp
```

---

## 📋 推奨アクション

### 即時対応（Priority 1）

1. **最小構成config.tomlの作成**
   - 動作確認済みの2サーバーのみを含む
   - ファイル名: `config-minimal.toml`

```toml
# Minimal Working Configuration
model = "gpt-5-codex"
experimental_use_rmcp_client = true
enable_web_search = true

[mcp_servers.codex]
args = ["mcp-server"]
command = "codex"
startup_timeout_sec = 60
env.OPENAI_API_KEY = "${OPENAI_API_KEY}"
env.RUST_LOG = "info"

[mcp_servers.codex-gemini-mcp]
args = []
command = "codex-gemini-mcp"
startup_timeout_sec = 60
env.PATH = "C:\\Users\\downl\\.cargo\\bin;${PATH}"
```

2. **起動スクリプトの作成**
```powershell
# start-codex-minimal.ps1
$env:CODEX_CONFIG = "$env:USERPROFILE\.codex\config-minimal.toml"
codex
```

---

### 中期対応（Priority 2）

1. **各MCPサーバーの個別デバッグ**
   - 手動起動テストの実施
   - エラーログの収集
   - 依存関係の確認

2. **環境変数の設定**
   - `BRAVE_API_KEY` の取得と設定
   - `GITHUB_TOKEN` の検証
   - その他必要なAPIキーの取得

3. **パッケージの事前インストール**
```powershell
# npxパッケージの事前キャッシュ
npx -y @modelcontextprotocol/server-brave-search --help
npx -y @modelcontextprotocol/server-filesystem --help
npx -y @modelcontextprotocol/server-github --help
```

---

### 長期対応（Priority 3）

1. **未実装サブコマンドの実装**
   - `codex supervisor` の実装
   - `codex research` の実装（`codex research` コマンドは存在するが、MCP版は未実装）
   - `codex agent` の実装（`codex agent-create` は存在）

2. **disabled フラグの修正**
   - Codex CLIのMCP設定読み込みロジックを修正
   - `disabled = true` が正しく機能するようにする

3. **タイムアウト値の動的調整**
   - 初回起動時は180秒
   - 2回目以降は30秒
   - パッケージキャッシュ有無の自動検出

---

## 🎯 結論

**現状**: 15個のMCPサーバー設定のうち、2個のみ正常動作（13.3%成功率）

**動作確認済み**:
1. ✅ codex (mcp-server) - 7ツール
2. ✅ codex-gemini-mcp - 1ツール（Google Search）

**推奨構成**: 
- 本番環境では最小構成（2サーバー）を使用
- 追加サーバーは個別にデバッグ後、段階的に追加

**次のステップ**:
1. ✅ 最小構成config.tomlの作成と検証
2. ⏳ uvx/npx系サーバーの個別デバッグ
3. ⏳ 環境変数の設定完了
4. ⏳ 未実装サブコマンドの実装検討

---

## 📊 統計情報

| カテゴリ | 成功 | 失敗 | 成功率 |
|---------|------|------|--------|
| Rust製（codex系） | 2 | 3 | 40.0% |
| Python製（uvx） | 0 | 3 | 0.0% |
| Node.js製（npx） | 0 | 7 | 0.0% |
| **合計** | **2** | **13** | **13.3%** |

**起動時間**:
- 成功サーバー平均: < 1秒
- 失敗サーバー平均: 60秒（タイムアウト）
- 全サーバー起動試行時間: 約13分

---

**作成日時**: 2025-10-25  
**担当者**: zapabob  
**Codexバージョン**: v0.49.0-zapabob.1  
**ステータス**: ✅ 診断完了

