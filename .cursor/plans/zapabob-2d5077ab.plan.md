<!-- 2d5077ab-069a-48a4-86ab-95beef4fe263 6d3ea0bf-01dd-401b-b594-017978529006 -->
# zapabob/codex 機能完成実装計画

## Phase 1: クイックウィン（2週間、43.75% → 60%）

### 1.1 サブエージェント3種追加（2-3時間）

既存8種類のテンプレートを参考に、残り3種類を追加。

**作成ファイル**:

- `.codex/agents/architect.yaml` - システム設計・アーキテクチャ分析専門
- `.codex/agents/executor.yaml` - 実装実行・コード生成専門  
- `.codex/agents/refactorer.yaml` - リファクタリング・最適化専門

**参考**: `code-reviewer.yaml`の構造（tools, policies, success_criteria）を踏襲し、各エージェントの専門性に合わせてカスタマイズ。

### 1.2 スコアリング機能実装（3-4時間）

`codex-rs/supervisor/src/`に新規ファイル`scoring.rs`を作成。

**実装内容**:

```rust
pub struct ScoringMetrics {
    test_pass_rate: f64,      // テスト成功率
    coverage_delta: f64,      // カバレッジ増減
    lint_score: f64,          // lint/type/secスコア
    performance_delta: f64,   // ベンチマーク差分
    change_risk: f64,         // 変更リスク（ファイル数/API変更）
    readability: f64,         // formatter適合/循環依存
}

pub fn calculate_score(metrics: &ScoringMetrics) -> f64;
pub fn rank_solutions(solutions: Vec<AgentResult>) -> Vec<(usize, f64)>;
```

**統合先**: `supervisor/src/lib.rs`の`coordinate_goal`に組み込み、`multi_agent_evaluator.rs`と連携。

### 1.3 合議統合実装（2-3時間）

`codex-rs/supervisor/src/consensus.rs`を新規作成。

**実装内容**:

- 複数エージェント結果の投票・スコアリング
- 最良案の自動選択
- Decision Logの生成（理由・スコア・根拠）

**既存活用**: `multi_agent_evaluator.rs`の`EvaluationScore`を拡張。

### 1.4 DeepResearch evidence JSON標準化（2-3時間）

`codex-rs/deep-research/src/evidence.rs`を拡張。

**実装内容**:

```rust
pub struct Evidence {
    title: String,
    url: String,
    published: Option<String>,
    quote: String,
    confidence: f64,
}

pub struct ResearchLog {
    query: String,
    timestamp: u64,
    sources: Vec<Evidence>,
    seed: Option<u64>,
}
```

**保存先**: `.codex/research/`ディレクトリに`{timestamp}_{query_hash}.json`形式で保存。

### 1.5 ロックCLI実装（1-2時間）

`codex-rs/cli/src/lock_cmd.rs`を新規作成。

**実装内容**:

- `codex lock status` - 現在のロック状態表示
- `codex lock remove [--force]` - ロック削除（強制オプション付き）

**既存活用**: `codex-rs/core/src/lock.rs`の`RepositoryLock`を使用。

**CLIルーティング**: `cli/src/main.rs`に新規サブコマンド追加。

---

## Phase 2: 長期計画詳細設計（4ヶ月）

### 2.1 Orchestratorサーバ実装（4週間）

#### Week 1-2: トランスポート層

**新規ファイル**:

- `codex-rs/orchestrator/src/transport/uds.rs` - Unix Domain Socket
- `codex-rs/orchestrator/src/transport/named_pipe.rs` - Windows Named Pipe
- `codex-rs/orchestrator/src/transport/tcp.rs` - TCP (127.0.0.1)
- `codex-rs/orchestrator/src/auth.rs` - HMAC-SHA256認証

**実装**:

- `.codex/secret`自動生成（初回起動時）
- トランスポート自動検出（UDS優先 → Pipe → TCP）
- ±5分時刻スキュー許容

#### Week 3-4: RPC APIサーバ

**新規ファイル**:

- `codex-rs/orchestrator/src/server.rs` - メインサーバー
- `codex-rs/orchestrator/src/queue.rs` - 単一ライタキュー（Tokio mpsc）
- `codex-rs/orchestrator/src/idempotency.rs` - 応答キャッシュ（10分TTL）
- `codex-rs/orchestrator/src/rpc/` - RPC v1.0全API実装
  - `lock.rs` - status, acquire, release
  - `status.rs` - get
  - `fs.rs` - read, write, patch
  - `vcs.rs` - diff, commit, push
  - `agent.rs` - register, heartbeat, list
  - `task.rs` - submit, cancel
  - `tokens.rs` - reportUsage, getBudget
  - `session.rs` - start, end
  - `pubsub.rs` - subscribe, unsubscribe

**エラー処理**:

- 409: preimage/base mismatch
- 429: queue full（retry_after付き）

**CLI統合**: `cli/src/main.rs`で自動起動ロジック追加。

### 2.2 Git戦略実装（3週間）

#### Week 1-2: Worktree競合モード

**新規ファイル**:

- `codex-rs/core/src/git/worktree.rs` - worktree自動管理
- `codex-rs/core/src/git/competition.rs` - 競合実行・スコアリング
- `codex-rs/core/src/git/auto_pr.rs` - 勝者の自動PR作成

**実装**:

```bash
codex run --worktree-competition --agents researcher,architect,executor
```

- 各エージェントが独立worktree/ブランチで実装
- スコアリングで最良案を選択
- 自動でPR作成、Decision Log付与

#### Week 3: Orchestrated Editモード

**新規ファイル**:

- `codex-rs/core/src/git/orchestrated_edit.rs` - intent-to-editロック
- `codex-rs/core/src/git/file_lock.rs` - ファイル粒度ロック

**実装**:

```bash
codex exec --playbook refactor/api --orchestrated-edit
```

- ファイル単位でロック取得
- 直列化された編集
- 競合ゼロ保証

### 2.3 TypeScript SDK実装（2週間）

#### Week 1: Protocol Client

**新規パッケージ**: `packages/codex-protocol-client/`

**実装内容**:

- トランスポート自動検出（UDS → Pipe → TCP）
- ジッタ付き再接続
- タイムアウト管理
- イベント購読
- 型付きRPCラッパー

**主要ファイル**:

- `src/client.ts` - メインクライアント
- `src/transport.ts` - トランスポート抽象化
- `src/types.ts` - 型定義
- `src/events.ts` - イベントハンドリング

#### Week 2: Reactフック

**実装**:

- `src/hooks/useProtocol.ts` - プロトコルクライアント接続
- `src/hooks/useProtocolEvent.ts` - イベント購読
- `src/hooks/useOrchestratorStatus.ts` - ステータス監視

### 2.4 GUI強化実装（2週間）

#### Week 1: キーボードショートカット

**修正ファイル**: `gui/src/components/Editor.tsx`（または該当コンポーネント）

**実装ショートカット**:

- `Cmd/Ctrl+Enter` → Run
- `Cmd/Ctrl+S` → Commit
- `Cmd/Ctrl+Shift+S` → Push
- `Cmd/Ctrl+D` → Diff
- `Cmd/Ctrl+Z` → Revert
- `?` → Help

**実装要件**:

- 入力中は無効化
- `aria-keyshortcuts`属性付与
- ツールチップ表示

#### Week 2: OrchestratorStatusDashboard

**新規コンポーネント**:

- `gui/src/components/OrchestratorStatusDashboard.tsx`
- `gui/src/hooks/useOrchestratorStatus.ts`

**実装内容**:

- ロック状態リアルタイム表示
- トークン使用量グラフ
- アクティブエージェント一覧
- `lock.changed`, `tokens.updated`イベント購読
- 5秒ポーリングフォールバック

### 2.5 Gemini OAuth 2.0統合（2週間）

#### Week 1: 認証プロバイダー実装

**新規ファイル**:

- `codex-rs/core/src/auth/gemini.rs` - GeminiAuthProvider
- `codex-rs/core/src/auth/oauth_pkce.rs` - OAuth 2.0/PKCE実装

**実装内容**:

- APIキーモード（既存維持）
- OAuth 2.0/PKCEモード（新規）
- geminicli検出・優先ロジック
- キーリング保存（フォールバック: `.codex/credentials.json` 0600）

**環境変数サポート**:

- `GEMINI_API_KEY`, `GOOGLE_AI_STUDIO_API_KEY`
- `GOOGLE_OAUTH_CLIENT_ID`
- `GCP_PROJECT_ID`, `VERTEX_REGION`

#### Week 2: CLI/GUI統合

**新規ファイル**:

- `codex-rs/cli/src/auth_gemini_cmd.rs` - CLI認証コマンド

**CLI実装**:

```bash
codex auth gemini login   # ブラウザ起動→OAuth→保存
codex auth gemini status  # 認証状態表示（マスク）
codex auth gemini logout  # 認証削除
```

**GUI実装**:

- Sign in/Sign out/Statusボタン
- RPC: `auth.status`, `auth.login.start`, `auth.logout`
- イベント: `auth.changed`

**config.toml設定**:

```toml
[auth.gemini]
mode = "api-key"           # "api-key" | "oauth"
provider = "ai_studio"     # "ai_studio" | "vertex"
prefer_cli = true
project = ""
region = ""
```

### 2.6 ドキュメント・テスト完成（2週間）

#### Week 1: ドキュメント作成

**新規ドキュメント**:

- `docs/SUBAGENTS.md` - サブエージェント使用ガイド
- `docs/DEEPRESEARCH.md` - DeepResearch使用ガイド
- `docs/WORKTREES.md` - Git worktree戦略ガイド
- `docs/protocol.md` - Orchestratorプロトコル仕様
- `docs/orchestration.md` - オーケストレーション詳細
- `docs/auth-gemini.md` - Gemini認証ガイド
- `docs/troubleshooting-locks.md` - ロックトラブルシューティング
- `docs/tokens.md` - トークン予算管理
- `docs/security.md` - セキュリティベストプラクティス

**更新ファイル**:

- `README.md` - ショートカット一覧、アーキテクチャ図更新
- `.env.sample` - 環境変数サンプル追加

#### Week 2: テスト実装

**Unit Tests**:

- `codex-rs/orchestrator/tests/lock_lifecycle.rs`
- `codex-rs/orchestrator/tests/token_budget.rs`
- `codex-rs/orchestrator/tests/hmac_auth.rs`
- `codex-rs/orchestrator/tests/idempotency.rs`
- `codex-rs/core/tests/auth/gemini_oauth.rs`

**Integration Tests**:

- `codex-rs/orchestrator/tests/integration/multi_instance_lock.rs`
- `codex-rs/orchestrator/tests/integration/fs_patch_conflict.rs`
- `codex-rs/orchestrator/tests/integration/queue_backpressure.rs`
- `codex-rs/core/tests/integration/gemini_auth_flow.rs`

**E2E Tests**:

- `tests/e2e/gui_shortcuts.rs`
- `tests/e2e/orchestrator_workflow.rs`
- `tests/e2e/worktree_competition.rs`

---

## 実装詳細ノート

### スコアリング実装の重要ポイント

既存の`multi_agent_evaluator.rs`を活用:

- `EvaluationScore`構造体を拡張
- `SimpleEvaluationStrategy`をベースに5指標スコアリング実装
- `MultiAgentEvaluator`と統合

### トークン予算の既存実装

`budgeter.rs`は既に以下を実装済み:

- `try_consume()` - 予算チェック付き消費
- `set_agent_limit()` - エージェント別上限設定
- `get_remaining()` - 残量取得

**追加要素**:

- 警告閾値イベント発火
- GUIへのAPI露出

### ロック機構の既存実装

`lock.rs`は既に以下を実装済み:

- `LockMetadata`構造体（PID/PPID/uid/hostname等）
- `acquire()` - ロック取得
- `release()` - ロック解放
- `is_lock_alive()` - ステール判定

**追加要素**:

- 原子的生成（O_EXCL）の強化
- TTL管理の完全実装
- CLI公開

### DeepResearchの既存実装

`deep-research/src/`は既に以下を実装済み:

- 複数検索プロバイダー統合（DuckDuckGo, Brave, Google, Bing, Gemini）
- `types.rs`に基本的な型定義
- `pipeline.rs`で検索→要約フロー

**追加要素**:

- Evidence構造体の標準化
- 引用管理の強化
- 再現性ログ保存（`.codex/research/`）

---

## マイルストーン

### M1: クイックウィン完了（2週間）

- サブエージェント11種類完備
- スコアリング・合議統合実装
- DeepResearch evidence JSON標準化
- ロックCLI実装
- **達成度**: 43.75% → 60%

### M2: Orchestrator基盤（4週間）

- トランスポート層（UDS/Pipe/TCP）
- HMAC認証
- 単一ライタキュー
- RPC v1.0全API
- **達成度**: 60% → 75%

### M3: Git戦略・フロントエンド（5週間）

- Worktree競合モード
- Orchestrated Editモード
- TypeScript SDK
- GUIショートカット・ダッシュボード
- **達成度**: 75% → 90%

### M4: Gemini OAuth・完成（4週間）

- OAuth 2.0/PKCE統合
- geminicli統合
- 全ドキュメント作成
- 全テスト実装
- リリースノート
- **達成度**: 90% → 100%

---

## 技術的考慮事項

### 1. Orchestratorサーバのアーキテクチャ

既存の`app-server/`実装を参考にしつつ、以下の差別化:

- **用途**: IDE統合用（app-server）vs 複数インスタンス調整用（orchestrator）
- **スコープ**: セッション単位（app-server）vs リポジトリ単位（orchestrator）
- **ライフサイクル**: アプリ同期（app-server）vs 独立デーモン（orchestrator）

### 2. Git戦略の既存リソース活用

`codex-rs/utils/git/`は既に以下を提供:

- `apply.rs` - パッチ適用
- `ghost_commits.rs` - ゴーストコミット
- `operations.rs` - 基本Git操作

これらを拡張してworktree管理を実装。

### 3. TypeScript SDKの既存リソース

`sdk/typescript/`は基本SDKを提供済み。

新規の`codex-protocol-client`は独立パッケージとして実装し、既存SDKと並存。

### 4. テスト戦略

既存のテスト構造（`core/tests/`, `cli/tests/`等）に倣い:

- `#[test]`でUnit
- `#[tokio::test]`でIntegration
- `assert_cmd`でE2E CLI
- Playwright/Puppeteerで E2E GUI

---

## リスク管理

### 高リスク項目

1. **Orchestratorサーバの複雑性** - 段階的実装、MVP優先
2. **Git worktree戦略の不確実性** - PoC検証、フォールバック設計
3. **OAuth実装の認証フロー** - geminicli優先でリスク軽減

### 緩和策

- 各Phase終了時に動作確認・ユーザーフィードバック
- 既存機能との後方互換性を常に維持
- 段階的リリース（feature flag活用）

---

## 完了基準

### クイックウィン（Phase 1）

- [ ] 11サブエージェント全てが動作
- [ ] スコアリングで最良案を自動選択
- [ ] DeepResearch結果が`.codex/research/`に保存
- [ ] `codex lock status/remove`が動作
- [ ] 全Unit testsがパス

### 長期計画（Phase 2-4）

- [ ] Orchestratorサーバが全RPCに応答
- [ ] 複数インスタンスで競合が発生しない（409適切に返却）
- [ ] worktree競合モードで3案から1案を自動PR
- [ ] GUIショートカットが全て動作
- [ ] Gemini OAuth 2.0でログイン・ログアウト
- [ ] 全E2E testsがパス
- [ ] 全ドキュメント完備

### To-dos

- [ ] サブエージェント3種追加（architect.yaml, executor.yaml, refactorer.yaml）
- [ ] スコアリング機能実装（supervisor/src/scoring.rs）
- [ ] 合議統合実装（supervisor/src/consensus.rs）
- [ ] DeepResearch evidence JSON標準化（deep-research/src/evidence.rs）
- [ ] ロックCLI実装（cli/src/lock_cmd.rs）
- [ ] Orchestratorトランスポート層実装（UDS/Pipe/TCP + HMAC認証）
- [ ] Orchestrator RPCサーバ実装（単一ライタキュー + 全API）
- [ ] Git worktree競合モード実装
- [ ] Git orchestrated editモード実装
- [ ] TypeScript protocol-client実装
- [ ] GUIショートカット実装
- [ ] OrchestratorStatusDashboard実装
- [ ] Gemini OAuth 2.0/PKCE実装
- [ ] CLI/GUI Gemini認証統合
- [ ] 全ドキュメント作成（9ファイル + README更新）
- [ ] 全テスト実装（Unit/Integration/E2E）