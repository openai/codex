# Cursor Composer Integration Guide

Codex AIオーケストレーション機能をCursor IDE Composerで使用するためのガイド。

## 概要

Cursor Composerから以下のCodex機能を直接呼び出せます:
- サブエージェント実行（@code-reviewer, @researcher等）
- Deep Research（@researcher）
- AIオーケストレーション（@supervisor）

## セットアップ

### 1. MCP設定ファイルの配置

`.cursor/mcp-config.json` をCursorの設定ディレクトリにコピー:

**Windows:**
```powershell
Copy-Item ".cursor/mcp-config.json" "$env:APPDATA\Cursor\User\globalStorage\mcp\settings.json"
```

**macOS/Linux:**
```bash
cp .cursor/mcp-config.json ~/.config/Cursor/User/globalStorage/mcp/settings.json
```

### 2. Cursor設定の確認

Cursor > Settings > MCP で以下が表示されることを確認:
- ✅ codex
- ✅ codex-subagent
- ✅ codex-deep-research

### 3. 環境変数の設定

`.cursor/settings.json` に追加:
```json
{
  "mcp.env": {
    "OPENAI_API_KEY": "sk-...",
    "GITHUB_TOKEN": "ghp_..."
  }
}
```

## 使用方法

### サブエージェント呼び出し

#### @code-reviewer - コードレビュー
```
@code-reviewer このファイルをレビューしてベストプラクティスを適用して

# または
@code-reviewer Review this authentication logic for security issues
```

**実行内容:**
- コード品質チェック
- ベストプラクティス違反の検出
- セキュリティ問題の指摘
- リファクタリング提案

#### @researcher - Deep Research
```
@researcher Rust async error handling patterns --depth 3

# または
@researcher React Server Components best practices
```

**実行内容:**
- 5+ソースから情報収集
- 引用付きレポート生成
- 矛盾検出と分析
- 実装例の提供

#### @test-gen - テスト生成
```
@test-gen このモジュールの包括的なテストを生成して

# または
@test-gen Generate unit and integration tests for auth module
```

**実行内容:**
- 単体テスト生成
- 統合テスト生成
- エッジケースのカバレッジ
- モック実装

#### @sec-audit - セキュリティ監査
```
@sec-audit このコードのセキュリティ脆弱性をチェック

# または
@sec-audit Audit this API endpoint for OWASP Top 10 vulnerabilities
```

**実行内容:**
- CVEスキャン
- 依存関係監査
- OWASP Top 10チェック
- セキュリティレポート生成

### AIオーケストレーション

#### @supervisor - タスク調整
```
@supervisor Implement user authentication with tests and security audit

# または
@supervisor Coordinate code review, testing, and security analysis for login feature
```

**実行内容:**
- タスク分析と分解
- 適切なエージェントを自動選択
- 並列/順次実行を最適化
- 結果の統合とレポート生成

**自動選択されるエージェント例:**
- `Implement user authentication` → code-reviewer, sec-audit
- `with tests` → test-gen
- `security audit` → sec-audit

## 高度な使用例

### 1. 選択範囲のレビュー

Cursorでコードを選択してから:
```
@code-reviewer 選択部分をレビューして最適化提案を
```

### 2. ファイル全体の包括的分析

```
@supervisor このファイル全体をレビュー、テスト生成、セキュリティ監査して
```

**実行フロー:**
1. Supervisorがタスク分析
2. code-reviewer, test-gen, sec-auditを並列実行
3. 結果を統合してレポート生成

### 3. Deep Researchによる調査

```
@researcher この実装パターンのベストプラクティスを調査して --depth 5
```

**実行フロー:**
1. 5+ソースから情報収集
2. 矛盾検出と検証
3. 引用付きレポート生成
4. 実装例の提供

### 4. 段階的な開発フロー

```
Step 1: @researcher OAuth 2.0 best practices for Express.js
Step 2: @code-reviewer Review current auth implementation
Step 3: @supervisor Refactor auth based on research and review
Step 4: @test-gen Generate comprehensive tests
Step 5: @sec-audit Final security audit
```

## Composer Tips

### 1. コンテキスト自動送信

Composerは自動的に以下を送信:
- 開いているファイル
- 選択範囲
- 最近の編集履歴

明示的なコンテキスト不要:
```
# ❌ 冗長
@code-reviewer Review src/auth.rs file

# ✅ 簡潔（auth.rsを開いている場合）
@code-reviewer Review this for security issues
```

### 2. 複数エージェントの連携

```
@researcher React hooks best practices
# [結果を確認]
@code-reviewer Apply the research findings to refactor this component
# [リファクタリング実施]
@test-gen Generate tests for the refactored component
```

### 3. 並列実行

```
@supervisor Run code review and security audit in parallel
```

Supervisorが自動的に:
- タスクを分解
- 並列実行可能か判定
- 最適な実行戦略を選択

## エラーハンドリング

### タイムアウト
デフォルトタイムアウト: 5分

```
# タイムアウト時の表示
⚠️ Supervisor execution timed out after 5m
Suggestion: Break down task into smaller steps
```

### リトライ
自動リトライ: 最大3回（指数バックオフ）

```
# リトライ時の表示
⚠️ Attempt 1/3 failed: connection error
🔄 Retrying after 1s...
✅ Attempt 2/3 succeeded
```

### エージェント選択エラー

```
# 存在しないエージェント
@unknown-agent Do something
❌ Error: Agent 'unknown-agent' not found
Available agents: researcher, code-reviewer, test-gen, sec-audit
```

## パフォーマンス最適化

### キャッシング

同じクエリの2回目以降は即座に応答:
```
@researcher Rust async patterns  # 初回: ~10秒
@researcher Rust async patterns  # 2回目: < 1秒 (キャッシュヒット)
```

### 並列実行

Supervisorは自動的に並列化:
```
@supervisor Review and test this module

# 実行:
# ┌─ code-reviewer (並列)
# └─ test-gen (並列)
# → 実行時間: max(reviewer_time, test-gen_time)
```

## トラブルシューティング

### 問題1: エージェントが応答しない

**解決方法:**
```bash
# MCPサーバーの状態確認
codex mcp list

# 再起動
codex mcp restart codex

# ログ確認
cat ~/.codex/logs/mcp-server.log
```

### 問題2: キャッシュが多すぎる

**解決方法:**
```bash
# キャッシュクリア
codex research --clear-cache

# または、プログラム的に
# (Deep Researchプロバイダーで)
provider.clear_cache().await;
```

### 問題3: タイムアウトが頻発

**解決方法:**
```toml
# ~/.codex/config.toml
[deep_research]
timeout_seconds = 600  # 10分に延長

[supervisor]
timeout_seconds = 600  # 10分に延長
```

## 統計とモニタリング

### キャッシュ統計
```rust
let (total, expired) = provider.get_cache_stats().await;
println!("Cache: {} total, {} expired", total, expired);
```

### 検索統計
```rust
let stats = provider.get_stats().await;
println!("Total searches: {}", stats.total_searches);
println!("Success rate: {:.1}%", 
    stats.successful_searches as f64 / stats.total_searches as f64 * 100.0
);
println!("Fallback rate: {:.1}%",
    stats.fallback_uses as f64 / stats.total_searches as f64 * 100.0
);
```

## ベストプラクティス

### 1. 適切なエージェント選択
- 単純なタスク: 直接エージェント指定（@code-reviewer）
- 複雑なタスク: Supervisor使用（@supervisor）

### 2. Deep Researchの活用
- 未知の技術調査: depth 3-5
- 既知の技術確認: depth 1-2
- 広範な調査: strategy exploratory

### 3. キャッシュの活用
- 同じクエリは再利用
- 定期的にexpired cache削除
- 大量クエリ前にキャッシュクリア

### 4. エラー対応
- タイムアウト: タスク分解
- リトライ失敗: 別エージェント使用
- キャッシュミス: 時間的余裕を持つ

## まとめ

Cursor Composerから Codex の強力な機能を直接利用可能:
- ✅ 8種類の特化エージェント
- ✅ AI オーケストレーション
- ✅ Deep Research
- ✅ 自動キャッシング
- ✅ リトライとタイムアウト管理
- ✅ 並列実行最適化

これにより、Claude Code を超える機能を Cursor IDE で実現。

