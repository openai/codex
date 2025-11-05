# zapabob/codex 独自機能リスト

**バージョン**: 0.48.0-zapabob.1  
**ベースコミット**: OpenAI/codex upstream/main (88abbf58)  
**作成日**: 2025-10-24

## 🚀 zapabob独自実装機能

### 1. Deep Research機能
**ディレクトリ**: `codex-rs/deep-research/`  
**機能**:
- マルチソース検索（Brave, DuckDuckGo, Google, Bing）
- MCP統合による実際の検索API利用
- フォールバックチェーン実装
- 検索統計トラッキング
- キャッシング機能

**ファイル**:
- `src/lib.rs` - メインライブラリ
- `src/mcp_search_provider.rs` - MCP検索プロバイダー
- `src/provider.rs` - プロバイダーtrait
- `src/types.rs` - 型定義

### 2. Gemini CLI MCP Server
**ディレクトリ**: `codex-rs/gemini-cli-mcp-server/`  
**機能**:
- Google Gemini AI統合
- OAuth 2.0認証（APIキー不要）
- Google Search Grounding
- MCPプロトコル対応

**ファイル**:
- `src/main.rs` - MCPサーバー実装

### 3. Supervisor機能
**ディレクトリ**: `codex-rs/supervisor/`  
**機能**:
- マルチエージェント協調
- タスク分解・実行計画生成
- 並列実行サポート
- 結果集約

**注意**: `codex-rs/core/src/orchestration/` に統合された新実装もあり

### 4. MCP Server機能拡張
**ディレクトリ**: `codex-rs/mcp-server/`  
**機能**:
- 7つのMCPツール実装
  - `codex` - メインセッション
  - `codex-reply` - 会話継続
  - `codex-supervisor` - マルチエージェント協調
  - `codex-deep-research` - 深堀り調査
  - `codex-subagent` - サブエージェント管理
  - `codex-custom-command` - カスタムコマンド
  - `codex-auto-orchestrate` - 自動オーケストレーション

### 5. サブエージェント定義
**ディレクトリ**: `.codex/agents/`  
**エージェント** (8個):
- `code-reviewer.yaml` - コードレビュー
- `codex-mcp-researcher.yaml` - MCP経由リサーチ
- `python-reviewer.yaml` - Python専用レビュー
- `researcher.yaml` - 深堀り調査
- `sec-audit.yaml` - セキュリティ監査
- `test-gen.yaml` - テスト生成
- `ts-reviewer.yaml` - TypeScript専用レビュー
- `unity-reviewer.yaml` - Unity専用レビュー

### 6. Orchestration機能
**ディレクトリ**: `codex-rs/core/src/orchestration/`  
**機能**:
- `auto_orchestrator.rs` - 自動オーケストレーション
- `collaboration_store.rs` - コラボレーション管理
- `conflict_resolver.rs` - コンフリクト解決
- `error_handler.rs` - エラーハンドリング
- `task_analyzer.rs` - タスク分析

### 7. 日本語音声通知機能
**ディレクトリ**: `zapabob/scripts/`  
**機能**:
- タスク完了時の音声通知（Marisa音声）
- PowerShellスクリプト実装
- フック統合（`config.toml`）

**ファイル**:
- `play-completion-sound.ps1` - 音声再生スクリプト
- `.codex/marisa_owattaze.wav` - 音声ファイル

### 8. 拡張されたエラーハンドリング
**ファイル**: `codex-rs/core/src/error.rs`  
**機能**:
- `TurnAborted` エラーに `dangling_artifacts` サポート
- `turn_aborted_with_artifacts()` ヘルパー関数
- `CancelErr` からの変換で artifacts 保持

### 9. 並行性向上
**ファイル**: 
- `codex-rs/core/src/state/session.rs`
- `codex-rs/core/src/conversation_history.rs`
- `codex-rs/core/src/codex.rs`

**機能**:
- `history_snapshot()` を `&self` に変更
- `contents()` を public(crate) に変更
- 不変ロックで並行アクセス可能

### 10. ツールオーケストレーション改善
**ファイル**: `codex-rs/core/src/tools/orchestrator.rs`  
**機能**:
- サンドボックス承認ロジックの改善
- `wants_no_sandbox_approval()` の正しい使用
- エラーハンドリングの明確化

## 📊 統計情報

| カテゴリ | 追加項目数 |
|---------|----------|
| 新規ディレクトリ | 4個 |
| エージェント定義 | 8個 |
| MCPツール | 7個 |
| コア機能拡張 | 5個 |

## 🎯 公式との差分

### zapabob独自 (公式にない機能)
1. ✅ Deep Research機能（全体）
2. ✅ Gemini CLI MCP Server
3. ✅ 8つのサブエージェント定義
4. ✅ 日本語音声通知機能
5. ✅ MCPサーバーツール拡張（7ツール）
6. ✅ Orchestration機能拡張

### 公式と共通 (マージ可能)
1. 🔄 コアCodex機能
2. 🔄 TUI実装
3. 🔄 CLI実装
4. 🔄 基本的なツール実装

### 公式の最新機能 (取り込み候補)
1. ⬇️ Followup feedback (#5663)
2. ⬇️ Bug fixes from upstream/main
3. ⬇️ 最新のプロトコル更新

## 📋 マージ戦略

### 推奨アプローチ: Cherry-pick + 独自機能保持

1. **公式の重要な修正のみ取り込み**
   - バグ修正
   - セキュリティパッチ
   - パフォーマンス改善

2. **独自機能は完全保持**
   - Deep Research
   - Gemini MCP
   - Orchestration
   - サブエージェント

3. **コンフリクト解決方針**
   - 公式の変更を優先（コア機能）
   - zapabob独自機能は保護
   - 両立可能な場合は統合

## 🔒 保護すべきファイル

### 絶対に保持
- `codex-rs/deep-research/` - 全体
- `codex-rs/gemini-cli-mcp-server/` - 全体
- `.codex/agents/` - 全体
- `zapabob/scripts/` - 全体
- `codex-rs/core/src/orchestration/` - zapabob拡張部分

### 慎重にマージ
- `codex-rs/mcp-server/` - 独自ツール保持
- `codex-rs/core/src/tools/` - 拡張機能保持
- `codex-rs/core/src/error.rs` - artifacts拡張保持
- `config.toml` - zapabob設定保持

## ✅ 次のステップ

1. upstream/mainの差分を確認
2. 重要な修正のみcherry-pick
3. 独自機能を保護しながらマージ
4. ビルド&テスト
5. セマンティックバージョンアップ

**戦略**: **保守的マージ** - 独自機能を最優先で保護

