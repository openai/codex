# 2025-10-23 Phase 2: rmcp統合最適化

## Summary
rmcp 0.8.3+のベストプラクティスに基づき、SupervisorツールハンドラーにTimeout、Retry、エラーハンドリングを実装。

## Phase 2.1: MCPツール統合のベストプラクティス適用

### 実装内容: supervisor_tool_handler.rs

#### 追加機能
1. **タイムアウト管理**
   - 定数: `SUPERVISOR_TIMEOUT = 300秒`（5分）
   - rmcpベストプラクティス準拠
   - 長時間実行タスクに対応

2. **リトライロジック**
   - 最大リトライ回数: `MAX_RETRY_ATTEMPTS = 3`
   - 指数バックオフ: `2^(attempt-1)秒`
   - ベース遅延: `BASE_RETRY_DELAY = 1秒`
   - リトライ可能エラーの判定実装

3. **エラーハンドリング強化**
   - 構造化ログ: `tracing`クレート使用
   - ログレベル: `info`, `debug`, `warn`, `error`
   - リクエストID追跡
   - 詳細なエラーメッセージ

#### コード変更

```rust
// Before (プレースホルダー実装)
pub async fn handle_supervisor_tool_call(
    _id: RequestId,
    arguments: Option<serde_json::Value>,
) -> CallToolResult {
    // 基本的なパラメータ検証のみ
    // エラーハンドリングなし
    // タイムアウトなし
}

// After (rmcp 0.8.3+ベストプラクティス)
pub async fn handle_supervisor_tool_call(
    id: RequestId,
    arguments: Option<serde_json::Value>,
) -> CallToolResult {
    info!("Supervisor tool call received (request_id: {:?})", id);
    
    // パラメータ検証 + ログ
    // execute_with_retry() でリトライ実行
    // 詳細なエラーハンドリング
}

async fn execute_with_retry(params: &SupervisorToolParam) -> anyhow::Result<String> {
    // タイムアウト付き実行
    // 指数バックオフでリトライ
    // リトライ可能エラーの判定
}

fn is_retryable_error(error: &anyhow::Error) -> bool {
    // timeout, connection, temporary, unavailable
}
```

### リトライ戦略

#### リトライ対象エラー
- ネットワークタイムアウト
- 接続エラー
- 一時的な障害
- サービス利用不可

#### 指数バックオフ
```
試行1: 即座実行
試行2: 1秒待機
試行3: 2秒待機
試行4: 4秒待機 (最大3回)
```

### ログ出力例

```rust
// 成功時
INFO  Supervisor tool call received (request_id: RequestId(...))
DEBUG Parsed supervisor parameters: goal=..., agents=[...]
DEBUG Supervisor execution attempt 1/3
INFO  Supervisor execution succeeded

// リトライ時
WARN  Supervisor execution attempt 1 failed: connection error
DEBUG Waiting 1s before retry
DEBUG Supervisor execution attempt 2/3
INFO  Supervisor execution succeeded

// 失敗時
ERROR Supervisor execution timed out after 300s
ERROR Supervisor execution failed after retries: ...
```

## rmcp 0.8.3+ベストプラクティス適用状況

### ✅ 実装済み
1. **タイムアウト管理**
   - 適切なタイムアウト値設定（5分）
   - tokio::time::timeout使用
   
2. **リトライメカニズム**
   - 指数バックオフ実装
   - リトライ可能エラーの判定
   - 最大リトライ回数の制限

3. **構造化ログ**
   - tracingクレート使用
   - 適切なログレベル
   - リクエストID追跡

4. **エラーハンドリング**
   - 詳細なエラーメッセージ
   - エラー分類（リトライ可能/不可能）
   - クライアントへの適切な情報提供

### 🔄 実装予定（Phase 2.2以降）
1. **接続プーリング**
   - rmcpクライアントの再利用
   - 接続キャッシング
   
2. **メトリクス収集**
   - 実行時間計測
   - 成功/失敗率追跡
   - リトライ頻度モニタリング

3. **レート制限**
   - API呼び出し頻度制御
   - バックプレッシャー処理

## 変更ファイル

### 修正
- `codex-rs/mcp-server/src/supervisor_tool_handler.rs`
  - タイムアウト定数追加
  - リトライロジック実装
  - エラーハンドリング強化
  - 構造化ログ追加

### 追加依存関係
```toml
# Cargo.tomlに既存
tracing = { workspace = true }
tokio = { workspace = true, features = ["time"] }
anyhow = { workspace = true }
```

## テスト計画（Phase 2.2）

### 単体テスト
```rust
#[tokio::test]
async fn test_supervisor_timeout() {
    // タイムアウト動作確認
}

#[tokio::test]
async fn test_supervisor_retry() {
    // リトライロジック確認
}

#[tokio::test]
async fn test_retryable_error_detection() {
    // エラー分類確認
}
```

### 実機テスト
```bash
# 正常系
codex delegate researcher --goal "test query"

# タイムアウトテスト
codex delegate slow-agent --goal "long running task"

# リトライテスト
# (ネットワーク不安定環境で実行)
```

## パフォーマンス考慮事項

### タイムアウト値の選定
- 5分: 長時間実行タスクに対応
- DeepResearch: 複数ソース検索で2-3分必要
- 並列エージェント実行: 複数エージェントの待機時間含む

### リトライ戦略
- 指数バックオフ: ネットワーク負荷軽減
- 最大3回: 無限ループ防止
- リトライ可能エラー判定: 無駄なリトライ削減

### ログオーバーヘッド
- DEBUGレベル: 開発時のみ
- INFOレベル: 本番環境でも有効
- 構造化ログ: パース可能なフォーマット

## 次のステップ: Phase 2.2

### 実機テストとフィードバック
1. 単一エージェント起動テスト
2. 複数エージェント並列実行テスト
3. DeepResearch統合テスト
4. エラーケース処理確認
5. パフォーマンス計測

### パフォーマンス計測項目
- 応答時間（平均、P50、P95、P99）
- リソース使用量（CPU、メモリ）
- リトライ発生率
- タイムアウト発生率
- 成功/失敗率

## Notes
- rmcp 0.8.3+仕様に準拠
- 本番環境での信頼性向上
- 観測可能性の向上（ログ、メトリクス）
- 段階的な拡張が可能な設計

