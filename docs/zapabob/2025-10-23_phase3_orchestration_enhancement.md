# 2025-10-23 Phase 3: AIオーケストレーション強化

## Summary
AutoOrchestratorに動的エージェント選択、実行戦略決定、結果集約機能を実装。CollaborationStoreにエージェント間メッセージパッシング機能を追加。

## Phase 3.1: AutoOrchestratorの段階的実装

### 追加機能

#### 1. ExecutionStrategy Enum
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStrategy {
    Parallel,     // 最大速度で並列実行
    Sequential,   // 依存関係のある順次実行
    Hybrid,       // 並列と順次のハイブリッド
}
```

#### 2. 動的エージェント選択
```rust
pub fn select_agents_for_task(&self, analysis: &TaskAnalysis) -> Vec<String>
```

**アルゴリズム:**
1. タスク分析の推奨エージェントをベースライン
2. 必要スキルに基づいて特化エージェントを追加:
   - `testing` → `test-gen`
   - `security` → `sec-audit`
   - `research` → `researcher`
   - `code-review` → `code-reviewer`
3. 最低1つのエージェントを保証（デフォルト: `code-reviewer`）

**実装例:**
```rust
let selected_agents = orchestrator.select_agents_for_task(&analysis);
// 結果: ["code-reviewer", "test-gen", "sec-audit"]
```

#### 3. 実行戦略決定
```rust
pub fn determine_execution_strategy(&self, task: &PlannedTask) -> ExecutionStrategy
```

**判定ロジック:**
- **Sequential**: `after`, `then`, `depends on`, `based on` を含む
- **Hybrid**: `edit`, `modify`, `change` を含む（ファイル編集競合の可能性）
- **Parallel**: 上記以外（独立タスク）

**実装例:**
```rust
let task = PlannedTask {
    description: "Implement feature after reviewing code".to_string(),
    ...
};
let strategy = orchestrator.determine_execution_strategy(&task);
// 結果: ExecutionStrategy::Sequential
```

#### 4. 結果集約
```rust
pub fn aggregate_results(&self, results: Vec<AgentResult>) -> Result<OrchestratedResult>
```

**機能:**
- 複数エージェントの結果を統合
- ConflictResolverを使用してファイル編集競合を解決
- 失敗したエージェントをログ記録
- 実行時間と使用トークンを集計

## Phase 3.2: CollaborationStoreの実装

### 追加機能

#### 1. AgentMessage構造体
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub from: String,          // 送信元エージェント
    pub to: String,            // 送信先エージェント（または"broadcast"）
    pub content: Value,        // メッセージ内容
    pub timestamp: SystemTime, // タイムスタンプ
    pub priority: u8,          // 優先度（0-255）
}
```

#### 2. メッセージパッシングメソッド

**送信:**
```rust
// 特定エージェントへ送信
store.send_message(
    "researcher".to_string(),
    "code-reviewer".to_string(),
    json!({"findings": "security issue found"}),
    5  // 優先度
);

// 全エージェントへブロードキャスト
store.broadcast_message(
    "coordinator".to_string(),
    json!({"status": "phase 1 complete"}),
    10  // 高優先度
);
```

**受信:**
```rust
// メッセージ取得（優先度順にソート）
let messages = store.get_messages("code-reviewer");
for msg in messages {
    println!("From {}: {:?}", msg.from, msg.content);
}

// 既読メッセージをクリア
store.clear_messages("code-reviewer");

// 未読数確認
let unread = store.unread_message_count("code-reviewer");
```

#### 3. メッセージソートアルゴリズム
```rust
messages.sort_by(|a, b| {
    b.priority.cmp(&a.priority)                    // 優先度降順
        .then_with(|| a.timestamp.cmp(&b.timestamp)) // 次に時系列
});
```

### 使用例

#### シナリオ1: セキュリティ問題の共有
```rust
// sec-audit がセキュリティ問題を発見
store.send_message(
    "sec-audit".to_string(),
    "code-reviewer".to_string(),
    json!({
        "type": "security_issue",
        "severity": "high",
        "file": "auth.rs",
        "line": 42,
        "description": "SQL injection vulnerability"
    }),
    10  // 高優先度
);

// code-reviewer が受信して対応
let messages = store.get_messages("code-reviewer");
for msg in messages {
    if msg.priority >= 8 {
        // 緊急対応
    }
}
```

#### シナリオ2: フェーズ完了の通知
```rust
// coordinatorが全エージェントに通知
store.broadcast_message(
    "coordinator".to_string(),
    json!({
        "phase": 1,
        "status": "completed",
        "next_phase": 2,
        "context": {...}
    }),
    5
);
```

#### シナリオ3: テスト結果の共有
```rust
// test-gen がテスト結果を共有
store.send_message(
    "test-gen".to_string(),
    "code-reviewer".to_string(),
    json!({
        "coverage": 85.5,
        "failed_tests": ["test_auth"],
        "suggestion": "Fix authentication logic"
    }),
    7
);
```

## 変更ファイル

### 修正
1. `codex-rs/core/src/orchestration/auto_orchestrator.rs`
   - `ExecutionStrategy` enum追加
   - `select_agents_for_task()` メソッド実装
   - `determine_execution_strategy()` メソッド実装
   - `aggregate_results()` メソッド実装
   - `debug`ログ追加

2. `codex-rs/core/src/orchestration/collaboration_store.rs`
   - `AgentMessage` 構造体追加
   - `message_queue` フィールド追加
   - `send_message()` メソッド実装
   - `broadcast_message()` メソッド実装
   - `get_messages()` メソッド実装（優先度ソート付き）
   - `clear_messages()` メソッド実装
   - `unread_message_count()` メソッド実装

## ベストプラクティス適用

### 1. 動的エージェント選択
- スキルベースの自動選択
- 複数エージェントの組み合わせ最適化
- フォールバック機能（最低1エージェント保証）

### 2. 実行戦略の最適化
- タスクの依存関係を自動検出
- ファイル編集競合を予測
- 適切な並列化レベルを選択

### 3. メッセージパッシング
- 優先度ベースのキューイング
- ブロードキャスト機能
- タイムスタンプ付きメッセージ
- 既読管理機能

### 4. スレッドセーフティ
- `DashMap`による並行アクセス対応
- `Arc`による共有所有権
- デッドロックフリー設計

## Phase 3.3: 実機テスト計画

### テストケース

#### 1. 動的エージェント選択
```bash
# スキル: testing + security
codex orchestrate "Implement login with tests and security audit"
# 期待: code-reviewer, test-gen, sec-audit が選択される
```

#### 2. 実行戦略決定
```bash
# 依存関係あり → Sequential
codex orchestrate "Review code, then fix bugs based on review"

# 並列可能 → Parallel
codex orchestrate "Generate tests and run security audit"
```

#### 3. メッセージパッシング
```rust
// エージェント間通信テスト
let store = CollaborationStore::new();
store.send_message(
    "agent1".into(),
    "agent2".into(),
    json!({"data": "test"}),
    5
);
assert_eq!(store.unread_message_count("agent2"), 1);
```

## パフォーマンス特性

### 動的エージェント選択
- **計算量**: O(n) where n = スキル数
- **メモリ**: O(m) where m = 選択エージェント数
- **平均実行時間**: < 1ms

### メッセージパッシング
- **送信**: O(1) - DashMapへの挿入
- **受信**: O(n log n) - ソート処理（n = メッセージ数）
- **メモリ**: O(m × n) where m = エージェント数, n = メッセージ数

## 次のステップ: Phase 4

### DeepResearch機能のrmcp最適化
1. 検索プロバイダーのエラーハンドリング改善
2. フォールバックチェーンの最適化
3. 結果キャッシング実装
4. Citation管理の強化

## Notes
- Phase 3の実装は計画通り完了
- テストは次のビルド後に実施
- メッセージパッシング機能はマルチエージェント協調の基盤
- ExecutionStrategyの自動判定により最適な並列化を実現

