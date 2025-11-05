# Codex Sub-Agents & Deep Research - Cursor 実装計画書

**ステータス**: Draft  
**作成日**: 2025-10-12 (JST)  
**対象**: Codex Core, CLI/IDE, Supervisor, Deep Research チーム  
**バージョン**: v0.1.0

---

## 📋 エグゼクティブサマリー

本ドキュメントは、`codex-main/codex-rs` ワークスペースにおけるサブエージェント機構と Deep Research 機能を本番品質で統合するための実装計画を定義します。既存の MVP 実装（M1 完了）を基に、M2～M4 フェーズで段階的に機能を拡充し、GA（General Availability）を目指します。

### 主要目標

1. **サブエージェント機構の本番化**: `.codex/agents/*.yaml` ベースのエージェント定義、トークン動的配分、並列実行、PR 分割を安定化
2. **Deep Research v1 の統合**: 計画生成→探索→反証→出典必須レポートのパイプラインを完成させ、軽量版フォールバックと MCP 連携を実装
3. **ガバナンスとセキュリティ**: Budgeter 強化、監査ログ永続化、権限ポリシー厳密化
4. **CLI/IDE/Web/GitHub/Slack 動線の拡張**: 既存インターフェースを壊さずプラガブルに機能を追加

### 現状サマリー

| コンポーネント | ステータス | 課題 |
|---------------|-----------|------|
| **AgentDefinition/Loader** | ✅ 実装完了 | キャッシュの TTL 管理なし |
| **TokenBudgeter** | ✅ 実装完了 | トークン消費の監査ログ未連携 |
| **AgentRuntime** | ⚠️ 部分実装 | MCP ツール連携が実験的 |
| **Deep Research (Planner)** | ✅ 実装完了 | 軽量版フォールバックのロジック改善余地 |
| **Deep Research (Pipeline)** | ⚠️ 部分実装 | 反証チェックの精度向上が必要 |
| **CLI (delegate)** | ✅ 実装完了 | エラーハンドリングの改善 |
| **CLI (research)** | ⚠️ 部分実装 | Gemini CLI 統合が実験的 |
| **Supervisor** | ✅ 実装完了 | Deep Research 結果との統合未完 |
| **監査ログ** | ⚠️ 部分実装 | 永続化ストレージ未設定 |

---

## 1. 背景とスコープ

### 1.0 フォーク戦略（zapabob/codex）

本プロジェクトは **OpenAI/codex の独自フォーク**として以下の方針で開発します：

#### 上流互換性の維持
- フォークは**既定で本家と同等挙動**（互換モード）を維持
- 追加機能は**プラグイン的に有効化**（設定/フラグ）、既存 API/CLI は破壊しない
- 差分はモジュール分離・DI（依存性注入）で**局所化**し、アップストリーム取り込みを容易化

#### 差別化機能（Core Features）
1. **Deep Research**: APIキー不要の検索フォールバック + 計画的調査
2. **サブエージェcント機構**: タスク分割／並列実行／役割別エージェント
3. **Gemini CLI統合**: Google Search Grounding 利用
4. **URLデコーダー**: DuckDuckGo リダイレクト対応
5. **MCP連携**: IDE統合（Cursor/Windsurf 等）

#### ターゲットペルソナ
- **個人開発者**: ローカルCLI/IDE補助、検索付き調査、軽量導入・無料運用志向
- **企業チーム**: CI連携、コードレビュー/テスト生成、自社ポリシー準拠、監査ログ
- **研究者/LLM開発者**: マルチエージェント実験、プロンプト/推論戦略の検証

### 1.1 対象サーフェス

- **CLI**: `codex delegate`, `codex research` コマンド
- **IDE**: VS Code / Cursor 拡張、コマンドパレット統合
- **Web**: Codex Web Dashboard（将来拡張）
- **GitHub**: `@codex` コメント連携、PR 自動レビュー
- **Slack**: エージェント進捗通知、結果サマリー投稿

### 1.2 非機能要件

| 要件 | 詳細 |
|------|------|
| **セキュリティ** | エージェント別権限境界、最小権限原則、シークレット自動除去、機密データの外部送信抑制 |
| **スケーラビリティ** | 並列エージェント実行、トークン動的配分、軽量版フォールバック、I/O重畳で2×以上の実効改善 |
| **監査対応** | 全エージェント実行の詳細ログ（JSONL）、トークン使用量追跡、タイムスタンプ記録、外部呼び出し記録 |
| **可用性** | プロバイダ多重化、フォールバック、キャッシュ、失敗時の劣化運転、エージェント障害時の自動リトライ |
| **互換性** | 既存 CLI/IDE 動線を壊さない、`openai/codex` との双方向同期可能、既定で互換モード |
| **再現性** | 重要経路はシード/温度管理、決定性の高い後処理 |
| **可観測性** | メトリクス（レイテンシ/エラー/トークン/外部呼数）、p95/p99、キャッシュヒット率 |

### 1.3 検索プロバイダ選択指針

Deep Research の検索プロバイダは以下の優先順位で選択されます：

| 優先度 | プロバイダ | 特徴 | API キー | 無料枠 | 備考 |
|-------|-----------|------|---------|--------|------|
| **1** | **SearxNG** | セルフホスト、合法・堅牢、可観測 | 不要 | 無制限 | 推奨（自前サーバー） |
| **2** | **Brave API** | 高品質、従量課金 | 必要 | Free枠あり | `BRAVE_API_KEY` |
| **3** | **Google CSE** | 高精度、従量課金 | 必要 | 100/日無料 | `GOOGLE_API_KEY` + `GOOGLE_CSE_ID` |
| **4** | **DuckDuckGo HTML** | APIキー不要、最終フォールバック | 不要 | 無制限 | 非公式（DOM変更リスクあり） |
| **5** | **Official/構造化** | Rust docs/SO等の公式ソース | 不要 | 無制限 | ドメイン限定 |

**Gemini CLI 統合（オプション）**: `--gemini` 指定時は Gemini CLI（Google Search Grounding）を最上位に配置。利用不可時は自動フォールバック。

#### 運用ポリシー
- **無料運用**: Brave Free/Google CSE の枠内で節流。超過は有償APIへ切替。
- **Bing 旧API**: 既定無効（退役考慮）。Azure Grounding 経由プラグ可能な設計。
- **規約順守**: 機密抑止・監査ログを標準で備え、企業導入を容易に。
- **RPS/Quota ガード**: レート制限・日次クォータ・Bot検出時バックオフを実装。

### 1.4 制約事項と緩和策

#### 既知制約
- **DuckDuckGo 非公式性**: DOM/挙動変更リスク
  - **緩和策**: 抽象化層＋複数プロバイダ＋キャッシュ（LRU+TTL）
- **Bing 旧APIの退役**: 既定OFF
  - **緩和策**: Azure Grounding 経由に切替可能な設計
- **コスト/規約**: 既定は無償枠内に節流
  - **緩和策**: 企業は有償API切替、環境変数で制御

#### 技術制約

- `CODEX_SANDBOX_*` 環境変数関連コードは変更対象外
- 破壊的シェルスクリプトの実行禁止
- 既存インターフェースの破壊的変更を回避

---

## 2. 既存ギャップ分析

以下に、コードレビューで特定された欠落点をファイルパスと行番号とともに列挙します。

| ファイルパス | 行番号 | 課題内容 | 優先度 | 担当候補 |
|-------------|--------|----------|--------|----------|
| `codex-rs/core/src/agents/runtime.rs` | 880-936 | Codex MCP Server 起動処理が実験的。エラーハンドリング強化必要 | High | Core チーム |
| `codex-rs/core/src/agents/runtime.rs` | 995-1123 | `execute_agent_with_codex_mcp` のツールコール検出が簡易実装 | High | Core チーム |
| `codex-rs/core/src/agents/runtime.rs` | 1206-1240 | `detect_tool_calls` のパーサーが脆弱（正規表現 based） | Medium | Core チーム |
| `codex-rs/core/src/agents/budgeter.rs` | 全体 | トークン消費の監査ログ未連携 | High | Core チーム |
| `codex-rs/core/src/agents/loader.rs` | 67-91 | キャッシュ TTL 管理なし、Hot Reload 未対応 | Medium | Core チーム |
| `codex-rs/deep-research/src/planner.rs` | - | 軽量版フォールバックのロジックが静的（動的調整必要） | Medium | Deep Research チーム |
| `codex-rs/deep-research/src/contradiction.rs` | - | 反証チェックの精度向上（信頼性スコア導入） | Low | Deep Research チーム |
| `codex-rs/deep-research/src/pipeline.rs` | - | Deep Research 結果と Supervisor の統合未完 | High | Supervisor チーム |
| `codex-rs/cli/src/research_cmd.rs` | 51-65 | Gemini CLI 統合が実験的、エラーハンドリング改善 | Medium | CLI チーム |
| `codex-rs/supervisor/src/lib.rs` | 67-90 | `coordinate_goal` が Deep Research 結果を利用していない | High | Supervisor チーム |
| `codex-rs/core/src/audit_log.rs` | - | 監査ログの永続化ストレージ未設定（現在メモリ内のみ） | High | Core チーム |
| `.codex/policies/` | - | 権限ポリシーファイルの実装欠如（net.allowlist, mcp.allowlist） | Medium | Security チーム |

---

## 3. 実装フェーズ別ロードマップ

### M1: サブエージェント MVP ✅ **完了**

**期間**: 2025-10-01 ~ 2025-10-10（実績）  
**目標**: サブエージェントの基本機能実装とテスト合格

#### 完了項目
- ✅ `AgentDefinition`, `AgentLoader`, `TokenBudgeter`, `AgentRuntime` 実装
- ✅ `.codex/agents/*.yaml` スキーマ定義（4 エージェント）
- ✅ `codex delegate` CLI コマンド実装
- ✅ 基本的なユニットテスト（20+ テスト）

#### 依存関係
- [x] M0 Foundation Intake の要件レビュー完了
- [x] `.codex/agents/` 初期テンプレート（code-reviewer / test-gen / sec-audit / researcher）
- [x] `codex-rs/core` モジュール分割（types.rs / loader.rs / budgeter.rs）準備

#### 成果物
- [x] `codex-rs/core/src/agents/` モジュール（types.rs, loader.rs, budgeter.rs, runtime.rs）
- [x] `.codex/agents/{researcher,test-gen,sec-audit,code-reviewer}.yaml`
- [x] `codex-rs/cli/src/delegate_cmd.rs`
- [x] `_docs/2025-10-10_サブエージェントDeepResearch実装.md`

---

### M2: Deep Research v1 統合 ⚠️ **進行中（60%）**

**期間**: 2025-10-12 ~ 2025-10-25（想定）  
**目標**: Deep Research パイプラインの完成と MCP 連携

#### コンポーネント更新

| コンポーネント | ファイルパス | 実装内容 | 工数 | 担当 |
|--------------|-------------|----------|------|------|
| **Research Planner** | `codex-rs/deep-research/src/planner.rs` | 動的軽量版フォールバックロジック実装 | M | Deep Research |
| **Contradiction Checker** | `codex-rs/deep-research/src/contradiction.rs` | 信頼性スコア導入、クロスバリデーション強化 | M | Deep Research |
| **Research Pipeline** | `codex-rs/deep-research/src/pipeline.rs` | Supervisor との統合インターフェース追加 | H | Deep Research + Supervisor |
| **URL Decoder** | `codex-rs/deep-research/src/url_decoder.rs` | DuckDuckGo リダイレクト（`uddg=`）デコード、HTMLエンティティ除去 | L | Deep Research |
| **Provider Fallback** | `codex-rs/deep-research/src/web_search_provider.rs` | SearxNG→Brave→CSE→DDG フォールバックチェーン実装 | H | Deep Research |
| **Cache Layer** | `codex-rs/deep-research/src/cache.rs` | LRU+TTL キャッシュ（Query→Results）、RPS/日次Quotaガード | M | Deep Research |
| **MCP Integration** | `codex-rs/mcp-client/src/client.rs` | Budgeter との連携、トークン追跡 | H | MCP チーム |
| **Gemini CLI Provider** | `codex-rs/deep-research/src/gemini_search_provider.rs` | エラーハンドリング改善、リトライロジック追加、タイムアウト制御 | M | Deep Research |
| **Research CLI** | `codex-rs/cli/src/research_cmd.rs` | 詳細なプログレス表示、中断/再開機能、プロバイダ選択（`--provider`） | M | CLI |

#### 依存関係
- [ ] M1 成果物の main 取り込みと CI パス確認
- [ ] 検索系 API キー（Brave / Google / Bing）および Gemini プロジェクトの利用許諾
- [ ] `codex mcp-server` v0.3+ の安定ビルド（MCP inspector で動作確認）
- [ ] Budgeter シミュレーションモード + OTel ダッシュボードのステージング環境

#### テスト/検証計画
- **ユニットテスト**: 各プロバイダーのモックテスト（Web, Gemini, MCP）
- **統合テスト**: E2E 研究フロー（計画→探索→レポート生成）
- **負荷テスト**: 高トラフィック時の軽量版フォールバック動作検証
- **セキュリティテスト**: API キー不正利用のエッジケース

#### リスクと緩和策
| リスク | 確率 | 影響 | 緩和策 |
|--------|------|------|--------|
| MCP ツール統合の遅延 | Medium | High | 段階的統合、モックプロバイダーで先行テスト |
| Gemini CLI の不安定性 | Low | Medium | フォールバックチェーン（Brave→Google→Bing→DDG）実装済み |
| トークン消費の予測精度 | Medium | Medium | Budgeter のシミュレーションモード追加 |

#### 完了条件
- [ ] 全 Deep Research プロバイダーが本番稼働可能
- [ ] Supervisor が Deep Research 結果を利用可能
- [ ] 軽量版フォールバックが自動起動（utilization > 80%）
- [ ] MCP クライアントが Budgeter とトークン情報を共有
- [ ] 統合テストスイートが全通過（カバレッジ 80% 以上）

#### 成果物
- [ ] `codex-rs/deep-research/src/pipeline.rs` 改修版
- [ ] `codex-rs/mcp-client/src/client.rs` Budgeter 連携版
- [ ] `codex-rs/supervisor/src/integrated.rs` Deep Research 統合モジュール
- [ ] `tests/integration/deep_research_e2e.rs` E2E テストスイート
- [ ] `docs/deep-research-integration.md` 統合ガイド

---

### M3: 統合 & ガバナンス ⏳ **未着手**

**期間**: 2025-10-26 ~ 2025-11-15（想定）  
**目標**: ガバナンス機能の実装と監査ログの永続化

#### コンポーネント更新

| コンポーネント | ファイルパス | 実装内容 | 工数 | 担当 |
|--------------|-------------|----------|------|------|
| **Budgeter 強化** | `codex-rs/core/src/agents/budgeter.rs` | 監査ログ連携、トークン消費の詳細記録 | H | Core |
| **監査ログ永続化** | `codex-rs/core/src/audit_log.rs` | SQLite/PostgreSQL ストレージ実装 | H | Core |
| **権限ポリシー** | `.codex/policies/{net,mcp}.allowlist` | 許可リストスキーマ定義、検証ロジック | M | Security |
| **エージェント Hot Reload** | `codex-rs/core/src/agents/loader.rs` | ファイル監視、キャッシュ自動更新 | M | Core |
| **トークン予測** | `codex-rs/core/src/agents/budgeter.rs` | プロンプトサイズからトークン数を推定 | M | Core |
| **ツールコールパーサー改善** | `codex-rs/core/src/agents/runtime.rs:1206-1240` | JSON Schema ベースのパーサー実装 | M | Core |

#### 依存関係
- [ ] M2 Deliverables の main 反映と Clippy / test パス
- [ ] SQLite / PostgreSQL への Seatbelt 互換アクセス許可
- [ ] `.codex/policies/` テンプレートとセキュリティチームのレビュー承認
- [ ] OTel Collector + Grafana (または Datadog) のステージング環境

#### テスト/検証計画
- **ユニットテスト**: 監査ログの CRUD、権限ポリシーのバリデーション
- **E2E テスト**: エージェント実行→監査ログ永続化→検証クエリ
- **セキュリティテスト**: 不正な権限要求のブロック、シークレット漏洩防止
- **パフォーマンステスト**: 監査ログ書き込みのスループット（> 1000 件/秒）

#### リスクと緩和策
| リスク | 確率 | 影響 | 緩和策 |
|--------|------|------|--------|
| 監査ログのストレージ容量 | Medium | High | ログローテーション、圧縮、アーカイブ機能実装 |
| 権限ポリシーの複雑化 | Low | Medium | デフォルトポリシーのテンプレート提供 |
| Hot Reload のパフォーマンス影響 | Low | Low | ファイル監視の間隔を調整可能に（デフォルト 5 秒） |

#### 完了条件
- [ ] 全エージェント実行が監査ログに記録される
- [ ] 監査ログが永続化ストレージに保存される（SQLite/PostgreSQL）
- [ ] 権限ポリシーが `.codex/policies/` から読み込まれる
- [ ] エージェント定義の Hot Reload が動作する
- [ ] トークン予測精度が ±10% 以内
- [ ] ツールコールパーサーが複雑な JSON をハンドル可能

#### 成果物
- [ ] `codex-rs/core/src/audit_log.rs` 永続化版
- [ ] `codex-rs/core/src/agents/budgeter.rs` 監査ログ連携版
- [ ] `.codex/policies/net.allowlist` / `.codex/policies/mcp.allowlist`
- [ ] `codex-rs/core/src/agents/loader.rs` Hot Reload 版
- [ ] `docs/governance-guide.md` ガバナンス運用ガイド
- [ ] `docs/audit-log-schema.md` 監査ログスキーマ定義

---

### M4: GA (General Availability) ⏳ **未着手**

**期間**: 2025-11-16 ~ 2025-12-15（想定）  
**目標**: 本番環境での GA、ドキュメント整備、エコシステム統合

#### コンポーネント更新

| コンポーネント | ファイルパス | 実装内容 | 工数 | 担当 |
|--------------|-------------|----------|------|------|
| **IDE 拡張（VS Code）** | `vscode-extension/src/subagents.ts` | サブエージェント実行 UI、進捗表示 | H | IDE |
| **IDE 拡張（Cursor）** | - | コマンドパレット統合、結果プレビュー | H | IDE |
| **GitHub Bot** | - | `@codex delegate`, `@codex research` コメント連携 | H | Integrations |
| **Slack 通知** | - | エージェント完了通知、レポート投稿 | M | Integrations |
| **Web Dashboard** | - | エージェント管理 UI、監査ログビューア | H | Web |
| **パフォーマンス最適化** | 全体 | 非同期処理の最適化、キャッシュ戦略 | M | Core |
| **ドキュメント整備** | `docs/` | ユーザーガイド、API リファレンス、チュートリアル | M | Docs |

#### 依存関係
- [ ] M1〜M3 の成果物が main / release ブランチに統合済み
- [ ] Zapabob ↔ OpenAI リリースウィンドウとコードフリーズ期間の調整
- [ ] ベータユーザー（CLI / IDE / Web / GitHub / Slack）の確定と NDA 手続き
- [ ] サポート体制（オンコール、Runbook、Incident Playbook）ドラフト

#### テスト/検証計画
- **総合テスト**: 全機能の統合テスト（CLI/IDE/Web/GitHub/Slack）
- **ユーザビリティテスト**: ベータユーザーによる実運用テスト
- **負荷テスト**: 本番環境を想定した高負荷シナリオ（100+ 並列エージェント）
- **セキュリティ監査**: 外部セキュリティ監査、ペネトレーションテスト

#### リスクと緩和策
| リスク | 確率 | 影響 | 緩和策 |
|--------|------|------|--------|
| 未発見のバグ | Medium | High | ベータテスト期間を 2 週間確保、迅速なパッチリリース |
| パフォーマンス問題 | Low | High | 事前に負荷テスト実施、スケーリング戦略を準備 |
| ドキュメント不足 | Medium | Medium | 早期からドキュメント作成、レビュープロセス導入 |

#### 完了条件
- [ ] 全サーフェス（CLI/IDE/Web/GitHub/Slack）で機能が利用可能
- [ ] ベータテストで Critical バグゼロ
- [ ] ドキュメントがレビュー完了、公開可能
- [ ] パフォーマンスベンチマークが目標値を達成
- [ ] セキュリティ監査で問題なし

#### 成果物
- [ ] `vscode-extension/` サブエージェント統合版
- [ ] `docs/user-guide.md`, `docs/api-reference.md`, `docs/tutorials/`
- [ ] GitHub Bot リポジトリ（`codex-github-bot`）
- [ ] Slack 通知サービス（`codex-slack-notifier`）
- [ ] Web Dashboard（`codex-web-dashboard`）
- [ ] リリースノート（`RELEASE_NOTES_v1.0.md`）

---

## 4. Budgeter & ガバナンス仕様

### 4.1 トークン管理（Budgeter 拡張）

#### 現状機能（M1 完了）
- 全体予算とエージェント別予算の管理
- `try_consume` による予算チェックと消費
- 予算の動的再配分（`rebalance`）
- 使用率に基づく軽量版フォールバック判定

#### M3 で追加する機能
```rust
// codex-rs/core/src/agents/budgeter.rs
impl TokenBudgeter {
    /// トークン消費を監査ログに記録
    pub fn consume_with_audit(&self, agent_name: &str, tokens: usize, context: &str) -> Result<bool> {
        let success = self.try_consume(agent_name, tokens)?;
        if success {
            log_audit_event(AuditEvent::new(
                agent_name.to_string(),
                AuditEventType::TokenConsumption {
                    tokens,
                    context: context.to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                },
            )).await?;
        }
        Ok(success)
    }

    /// プロンプトサイズからトークン数を推定
    pub fn estimate_tokens(&self, prompt: &str) -> usize {
        // 簡易推定: 4 文字 = 1 トークン（英語基準）
        // TODO: tokenizer ライブラリ (tiktoken-rs) 導入検討
        (prompt.len() as f64 / 4.0).ceil() as usize
    }
}
```

### 4.2 監査ログの永続化

#### スキーマ定義
```sql
-- codex-rs/core/migrations/001_audit_log.sql
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    agent_name TEXT NOT NULL,
    event_type TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    data JSON NOT NULL,
    INDEX idx_agent_name (agent_name),
    INDEX idx_timestamp (timestamp)
);
```

#### 実装（SQLite ベース）
```rust
// codex-rs/core/src/audit_log/storage.rs
use rusqlite::{Connection, params};

pub struct AuditLogStorage {
    conn: Arc<Mutex<Connection>>,
}

impl AuditLogStorage {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(include_str!("../migrations/001_audit_log.sql"))?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub async fn write_event(&self, event: &AuditEvent) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO audit_log (event_id, agent_name, event_type, timestamp, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                event.event_id,
                event.agent_name,
                format!("{:?}", event.event_type),
                chrono::Utc::now().to_rfc3339(),
                serde_json::to_string(&event)?
            ],
        )?;
        Ok(())
    }

    pub async fn query_by_agent(&self, agent_name: &str, limit: usize) -> Result<Vec<AuditEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT data FROM audit_log WHERE agent_name = ?1 ORDER BY timestamp DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![agent_name, limit], |row| {
            let json: String = row.get(0)?;
            Ok(serde_json::from_str(&json).unwrap())
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
```

### 4.3 権限ポリシー（`.codex/policies/`）

#### ファイル構造
```
.codex/
├── policies/
│   ├── net.allowlist          # ネットワーク許可リスト
│   ├── mcp.allowlist          # MCP ツール許可リスト
│   └── filesystem.allowlist   # ファイルシステム許可リスト
```

#### `net.allowlist` 例
```yaml
# .codex/policies/net.allowlist
version: "1.0"
default_policy: deny

allowlist:
  - domain: "*.github.com"
    protocols: ["https"]
  - domain: "api.openai.com"
    protocols: ["https"]
  - domain: "localhost"
    ports: [8080, 3000]
    protocols: ["http", "https"]
```

#### `mcp.allowlist` 例
```yaml
# .codex/policies/mcp.allowlist
version: "1.0"
default_policy: deny

allowed_tools:
  - name: "codex_read_file"
    description: "ファイル読み取り（安全）"
    risk_level: "low"
  - name: "codex_grep"
    description: "パターン検索（安全）"
    risk_level: "low"
  - name: "codex_codebase_search"
    description: "セマンティック検索（安全）"
    risk_level: "low"
  - name: "codex_apply_patch"
    description: "パッチ適用（要注意）"
    risk_level: "medium"
    requires_approval: true
  - name: "codex_shell"
    description: "シェルコマンド実行（危険）"
    risk_level: "high"
    requires_approval: true
    restricted_agents: ["sec-audit"]  # 特定エージェントのみ許可
```

#### 実装（ポリシー検証）
```rust
// codex-rs/core/src/agents/policy.rs
pub struct PolicyManager {
    net_policy: NetPolicy,
    mcp_policy: McpPolicy,
}

impl PolicyManager {
    pub fn load_from_dir(policies_dir: &Path) -> Result<Self> {
        let net_policy = NetPolicy::load(policies_dir.join("net.allowlist"))?;
        let mcp_policy = McpPolicy::load(policies_dir.join("mcp.allowlist"))?;
        Ok(Self { net_policy, mcp_policy })
    }

    pub fn check_net_access(&self, agent: &AgentDefinition, url: &str) -> Result<bool> {
        self.net_policy.is_allowed(agent, url)
    }

    pub fn check_mcp_tool(&self, agent: &AgentDefinition, tool_name: &str) -> Result<bool> {
        self.mcp_policy.is_allowed(agent, tool_name)
    }
}
```

### 4.4 Agent Manifest スキーマ計画

#### フィールド定義（`docs/agent-manifest-schema.md` に集約）
| フィールド | 型 | 必須 | 説明 | 備考 |
|-----------|----|------|------|------|
| `name` | string | ✅ | エージェント識別子（CLI/IDE 表示名） | スネークケース + 英数字 |
| `goal` | string | ✅ | エージェントの目的を 1 文で表記 | i18n 対応は付録 B で管理 |
| `instructions` | multiline string | ✅ | システムプロンプト（Markdown 可） | 4KB 超で Budgeter が警告発火 |
| `tools` | map | ✅ | `mcp` / `fs` / `net` / `shell` 設定群 | allow/deny 記述を厳守 |
| `policies` | map | ✅ | `context` / `secrets` / `sandbox` 設定 | retention は `job` / `session` / `persistent` |
| `success_criteria` | list<string> | ✅ | 完了判定チェックリスト | 5 項目以内を推奨 |
| `artifacts` | list<string> | ⬜ | 出力成果物パス | CLI の `--artifact` と連携 |
| `telemetry_tags` | map | ⬜ | 監査ログ/コスト分析用メタデータ | 未指定時は runtime が補完 |

#### スキーマ運用ロードマップ
- **M1（完了）**: `.codex/agents/*.yaml` 雛形確定、`AgentManifest` 構造体を `codex-rs/core/src/agents/types.rs` に追加。
- **M2**: JSON Schema v7 を `schema/agent_manifest.json` として公開し、`codex validate-agent <path>` CLI と loader バリデーションを実装。
- **M3**: IDE/CLI の補完とホットリロードエラー表示を統合し、`docs/agent-manifest-schema.md` を英日併記で発行。
- **M4**: 署名付きエージェント配布（Marketplace/API）仕様をドラフト化し、監査ログ・Budgeter に `telemetry_tags` を連携。

#### チェックリスト
- [ ] `schema/agent_manifest.json` を CI (`cargo schema-check`) に組み込み（M2）
- [ ] `loader.rs` でスキーマ違反を構造化エラーとして出力（M2）
- [ ] `docs/agent-manifest-schema.md` を公開し upstream/fork で同期（M3）
- [ ] IDE/CLI の補完リストをスキーマ生成に切替（M3）
- [ ] 監査ログに `manifest_version` と `telemetry_tags` を保存（M4）

---

## 5. Deep Research 統合詳細

### 5.1 Supervisor との統合

#### 現状の問題
- `Supervisor::coordinate_goal` が Deep Research 結果を利用していない（`codex-rs/supervisor/src/lib.rs:67-90`）
- Deep Research が独立して実行され、Supervisor の計画に組み込まれない

#### M2 での実装
```rust
// codex-rs/supervisor/src/integrated.rs
use codex_deep_research::{DeepResearcher, DeepResearcherConfig, ResearchStrategy};

impl Supervisor {
    /// Deep Research を使ってゴールを分析し、計画を生成
    pub async fn analyze_goal_with_research(
        &self,
        goal: &str,
        research_depth: u8,
    ) -> Result<Plan> {
        // 1. Deep Research でトピックを調査
        let config = DeepResearcherConfig {
            max_depth: research_depth,
            max_sources: 10,
            strategy: ResearchStrategy::Focused,
        };
        let researcher = DeepResearcher::new(config, self.research_provider.clone());
        let report = researcher.research(goal).await?;

        // 2. Research 結果を Plan に統合
        let mut plan = planner::analyze_goal(goal)?;
        
        // 3. Research の Findings を Step の Context に追加
        for finding in &report.findings {
            for step in &mut plan.steps {
                if step.description.contains(&finding.content) {
                    step.context.insert(
                        "research_finding".to_string(),
                        finding.content.clone()
                    );
                    step.context.insert(
                        "confidence".to_string(),
                        finding.confidence.to_string()
                    );
                }
            }
        }

        // 4. Sources を Plan の Metadata に保存
        plan.metadata.insert(
            "research_sources".to_string(),
            serde_json::to_string(&report.sources)?
        );

        Ok(plan)
    }
}
```

### 5.2 サブクエリ計画の改善

#### 現状
- `ResearchPlanner::generate_plan` が静的なロジック（`codex-rs/deep-research/src/planner.rs`）
- サブクエリが単純な分割で、動的調整なし

#### M2 での改善
```rust
// codex-rs/deep-research/src/planner.rs
impl ResearchPlanner {
    /// 動的にサブクエリを生成（LLM ベース）
    pub async fn generate_plan_dynamic(
        topic: &str,
        depth: u8,
        breadth: usize,
        model_client: &ModelClient,
    ) -> Result<ResearchPlan> {
        // LLM にサブクエリ生成を依頼
        let prompt = format!(
            "Generate {breadth} focused sub-queries for researching: {topic}\n\
             Each query should cover a distinct aspect.\n\
             Output as JSON array of strings."
        );

        let response = model_client.generate_text(&prompt).await?;
        let sub_queries: Vec<String> = serde_json::from_str(&response)?;

        // 評価基準も LLM で生成
        let criteria_prompt = format!(
            "For research on '{topic}', list 5 success criteria to evaluate findings.\n\
             Output as JSON array of strings."
        );
        let criteria_response = model_client.generate_text(&criteria_prompt).await?;
        let evaluation_criteria: Vec<String> = serde_json::from_str(&criteria_response)?;

        Ok(ResearchPlan {
            main_topic: topic.to_string(),
            sub_queries,
            evaluation_criteria,
            stop_conditions: StopConditions {
                max_depth: depth,
                max_sources: breadth * 3,
                min_confidence: 0.7,
            },
            evidence_depth: depth,
        })
    }
}
```

### 5.3 軽量版フォールバック

#### 現状
- `ResearchPlanner::downgrade_to_lightweight` が単純な breadth 削減
- トークン消費の予測なし

#### M2 での改善
```rust
// codex-rs/deep-research/src/planner.rs
impl ResearchPlanner {
    /// トークン予算に基づいて動的にプランを調整
    pub fn adjust_plan_for_budget(
        plan: &ResearchPlan,
        available_tokens: usize,
    ) -> ResearchPlan {
        // トークン消費の推定
        let estimated_tokens_per_query = 1500; // 平均値
        let max_queries = available_tokens / estimated_tokens_per_query;

        let adjusted_sub_queries = if plan.sub_queries.len() > max_queries {
            // 優先度の高いクエリのみを選択
            plan.sub_queries.iter().take(max_queries).cloned().collect()
        } else {
            plan.sub_queries.clone()
        };

        ResearchPlan {
            main_topic: plan.main_topic.clone(),
            sub_queries: adjusted_sub_queries,
            evaluation_criteria: plan.evaluation_criteria.clone(),
            stop_conditions: StopConditions {
                max_depth: plan.stop_conditions.max_depth.min(2), // 深度を削減
                max_sources: plan.stop_conditions.max_sources.min(max_queries * 2),
                min_confidence: plan.stop_conditions.min_confidence,
            },
            evidence_depth: plan.evidence_depth.min(2),
        }
    }
}
```

### 5.4 URL デコーダー（DuckDuckGo 対応）

#### 実装（M2）
```rust
// codex-rs/deep-research/src/url_decoder.rs

/// DuckDuckGo のリダイレクトURL（`duckduckgo.com/l/?uddg=...`）を実URLに復元
pub fn decode_duckduckgo_url(url: &str) -> String {
    if url.contains("duckduckgo.com/l/?uddg=") {
        if let Some(start_idx) = url.find("uddg=") {
            let encoded = &url[start_idx + 5..];
            // `&amp;` で区切られた最初の部分を取得
            let encoded = if let Some(amp_idx) = encoded.find("&amp;") {
                &encoded[..amp_idx]
            } else {
                encoded
            };
            // URLデコード
            if let Ok(decoded) = urlencoding::decode(encoded) {
                return decoded.to_string();
            }
        }
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_duckduckgo_url() {
        let input = "https://duckduckgo.com/l/?uddg=https%3A%2F%2Fgithub.com%2Fopenai%2Fcodex&amp;rut=abc";
        let expected = "https://github.com/openai/codex";
        assert_eq!(decode_duckduckgo_url(input), expected);
    }
}
```

### 5.5 プロバイダフォールバックチェーン

#### 実装（M2）
```rust
// codex-rs/deep-research/src/web_search_provider.rs

pub struct WebSearchProvider {
    providers: Vec<Box<dyn SearchProvider>>,
    cache: Arc<Mutex<LruCache<String, Vec<SearchResult>>>>,
    rate_limiter: Arc<RateLimiter>,
}

impl WebSearchProvider {
    pub fn new() -> Self {
        let mut providers: Vec<Box<dyn SearchProvider>> = Vec::new();
        
        // 優先順位順にプロバイダを登録
        if let Ok(searx_url) = std::env::var("SEARXNG_URL") {
            providers.push(Box::new(SearxNGProvider::new(searx_url)));
        }
        if std::env::var("BRAVE_API_KEY").is_ok() {
            providers.push(Box::new(BraveSearchProvider::new()));
        }
        if std::env::var("GOOGLE_API_KEY").is_ok() && std::env::var("GOOGLE_CSE_ID").is_ok() {
            providers.push(Box::new(GoogleCSEProvider::new()));
        }
        // DuckDuckGo は常に利用可能（APIキー不要）
        providers.push(Box::new(DuckDuckGoProvider::new()));
        
        Self {
            providers,
            cache: Arc::new(Mutex::new(LruCache::new(100))),
            rate_limiter: Arc::new(RateLimiter::new(10, Duration::from_secs(1))),
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        // キャッシュチェック
        if let Some(cached) = self.cache.lock().unwrap().get(query) {
            return Ok(cached.clone());
        }

        // プロバイダを順に試行
        for provider in &self.providers {
            // レート制限チェック
            self.rate_limiter.wait().await;

            match provider.search(query).await {
                Ok(results) if !results.is_empty() => {
                    // キャッシュに保存
                    self.cache.lock().unwrap().put(query.to_string(), results.clone());
                    return Ok(results);
                }
                Ok(_) => {
                    warn!("Provider {} returned no results", provider.name());
                    continue;
                }
                Err(e) => {
                    warn!("Provider {} failed: {}", provider.name(), e);
                    continue;
                }
            }
        }

        anyhow::bail!("All search providers failed for query: {}", query)
    }
}
```

### 5.6 MCP ツール連携

#### 現状
- `McpSearchProvider` が DuckDuckGo のみ対応
- Budgeter とトークン情報を共有していない

#### M2 での実装
```rust
// codex-rs/mcp-client/src/client.rs
impl McpClient {
    /// トークン予算を考慮したツール呼び出し
    pub async fn call_tool_with_budget(
        &self,
        tool_name: String,
        args: Option<serde_json::Value>,
        budgeter: &Arc<TokenBudgeter>,
        agent_name: &str,
    ) -> Result<serde_json::Value> {
        // 推定トークン数（ツールごとに異なる）
        let estimated_tokens = match tool_name.as_str() {
            "codex_read_file" => 500,
            "codex_grep" => 300,
            "codex_codebase_search" => 1000,
            _ => 500,
        };

        // 予算チェック
        if !budgeter.try_consume(agent_name, estimated_tokens)? {
            anyhow::bail!("Token budget exceeded for tool call: {}", tool_name);
        }

        // ツール実行
        let result = self.call_tool(tool_name.clone(), args, Some(Duration::from_secs(30))).await?;

        // 実際のトークン数を監査ログに記録（将来拡張）
        info!("MCP tool '{}' consumed ~{} tokens", tool_name, estimated_tokens);

        Ok(result)
    }
}
```

---

## 6. ドキュメント/リリース計画

### 6.1 ドキュメント構成

| ドキュメント | 説明 | 対象読者 |
|------------|------|---------|
| `docs/user-guide.md` | エンドユーザー向けガイド | 開発者全般 |
| `docs/api-reference.md` | Rust API ドキュメント | Codex コントリビューター |
| `docs/governance-guide.md` | ガバナンス運用ガイド | エンタープライズ管理者 |
| `docs/audit-log-schema.md` | 監査ログスキーマ定義 | コンプライアンス担当 |
| `docs/tutorials/` | ステップバイステップチュートリアル | 初心者 |
| `docs/deep-research-integration.md` | Deep Research 統合ガイド | 内部開発者 |
| `docs/cursor-implementation-plan.md` | 本ドキュメント | 実装リード |

### 6.2 リリースノート構成

#### M2 リリース（v0.47.0）
```markdown
# Codex v0.47.0 - Deep Research v1 統合

## 🎯 主要機能
- Deep Research パイプライン完成（計画→探索→反証→レポート）
- MCP ツール連携（Budgeter とトークン共有）
- Gemini CLI プロバイダー安定化

## 🐛 バグ修正
- `AgentRuntime::execute_agent_with_codex_mcp` のツールコール検出改善
- `ResearchPlanner` の軽量版フォールバックが動的調整に対応
- トークン予測精度の向上（±10% 以内）

## 📊 パフォーマンス
- Deep Research 実行速度 20% 向上
- 並列エージェント実行時のメモリ使用量 15% 削減

## 🔗 関連リソース
- [Deep Research 統合ガイド](docs/deep-research-integration.md)
- [チュートリアル: 初めての Deep Research](docs/tutorials/deep-research-tutorial.md)
```

### 6.3 エコシステム同期

#### OpenAI 本家 (`openai/codex`) との同期戦略
1. **機能フラグ**: `CODEX_AGENT_RUNTIME`, `CODEX_DEEP_RESEARCH` で段階的有効化
2. **設定互換性**: `.codex/agents/*.yaml` スキーマを共通化
3. **PR 戦略**: zapabob/codex で安定化後、openai/codex へ逆流プルリク
4. **ドキュメント**: 両リポジトリで共通のドキュメントを参照

---

## 7. Open Questions

以下は実装中に解決すべき未確定事項です。

### 7.1 アーキテクチャ
- **Q1**: Supervisor が Deep Research を常に実行すべきか、オプトインか？
  - **提案**: 環境変数 `CODEX_AUTO_RESEARCH=1` でオプトイン
- **Q2**: エージェント間でコンテキストを共有する仕組みは？
  - **提案**: 共有メモリストア（`shared_context: HashMap<String, Value>`）を AgentRuntime に追加

### 7.2 トークン管理
- **Q3**: エージェント間でトークン予算を競合（bidding）させるべきか？
  - **提案**: 初期は centralized allocation、将来拡張で bidding 導入
- **Q4**: トークン予測の精度をどこまで高めるか？
  - **提案**: M3 で tiktoken-rs 導入、±5% 以内を目標

### 7.3 セキュリティ
- **Q5**: 権限ポリシー違反時の動作は？（エラー or 警告）
  - **提案**: デフォルトはエラー、`--allow-violations` フラグで警告に変更可
- **Q6**: 監査ログの保持期間は？
  - **提案**: デフォルト 90 日、設定で変更可能（`audit_log_retention_days`）

### 7.4 UX
- **Q7**: IDE でエージェントの進捗をどう表示するか？
  - **提案**: VS Code のタスクプログレス API を使用、サイドパネルに詳細表示
- **Q8**: エージェント実行の中断/再開機能は必要か？
  - **提案**: M4 で実装、`codex delegate --resume <task_id>` コマンド追加

---

## 8. 付録

### 8.1 実装チェックリスト（M2）

#### Deep Research
- [ ] `planner.rs`: 動的軽量版フォールバック実装
- [ ] `contradiction.rs`: 信頼性スコア導入
- [ ] `pipeline.rs`: Supervisor 統合インターフェース追加
- [ ] `url_decoder.rs`: DuckDuckGo リダイレクトデコーダー実装
- [ ] `web_search_provider.rs`: プロバイダフォールバックチェーン（SearxNG→Brave→CSE→DDG）
- [ ] `cache.rs`: LRU+TTL キャッシュ実装、RPS/Quotaガード
- [ ] `gemini_search_provider.rs`: エラーハンドリング改善、タイムアウト制御
- [ ] テスト: E2E 研究フロー（10+ テストケース）
- [ ] テスト: URLデコーダー（DuckDuckGo形式）
- [ ] テスト: プロバイダフォールバック（全パターン）

#### MCP 統合
- [ ] `mcp-client/client.rs`: Budgeter 連携実装
- [ ] `mcp-client/client.rs`: トークン追跡ロギング
- [ ] テスト: MCP ツール呼び出しのモックテスト

#### CLI
- [ ] `research_cmd.rs`: プログレス表示改善
- [ ] `research_cmd.rs`: 中断/再開機能（基本版）
- [ ] テスト: CLI 統合テスト

#### Supervisor
- [ ] `supervisor/integrated.rs`: Deep Research 統合
- [ ] `supervisor/lib.rs`: `analyze_goal_with_research` 実装
- [ ] テスト: Supervisor + Deep Research E2E

### 8.2 実装チェックリスト（M3）

#### Budgeter
- [ ] `budgeter.rs`: `consume_with_audit` 実装
- [ ] `budgeter.rs`: `estimate_tokens` 実装
- [ ] テスト: 監査ログ連携テスト

#### 監査ログ
- [ ] `audit_log/storage.rs`: SQLite ストレージ実装
- [ ] `audit_log/storage.rs`: クエリ API 実装
- [ ] マイグレーション: `001_audit_log.sql`
- [ ] テスト: ストレージ CRUD テスト

#### 権限ポリシー
- [ ] `.codex/policies/net.allowlist` 定義
- [ ] `.codex/policies/mcp.allowlist` 定義
- [ ] `agents/policy.rs`: PolicyManager 実装
- [ ] テスト: ポリシー検証テスト

#### Agent Loader
- [ ] `loader.rs`: Hot Reload 実装（ファイル監視）
- [ ] `loader.rs`: キャッシュ TTL 管理
- [ ] テスト: Hot Reload テスト

#### Agent Runtime
- [ ] `runtime.rs:1206-1240`: ツールコールパーサー改善
- [ ] テスト: 複雑な JSON パース

### 8.3 用語集

| 用語 | 説明 |
|------|------|
| **Sub-Agent** | 特定タスクに特化したエージェント（researcher, test-gen など） |
| **Deep Research** | 計画→探索→反証→レポートの一連のリサーチパイプライン |
| **Budgeter** | トークン予算を管理するコンポーネント |
| **MCP** | Model Context Protocol（外部ツール統合プロトコル） |
| **Supervisor** | 複数エージェントを調整するオーケストレーター |
| **Lightweight Fallback** | トークン不足時の軽量版研究モード |
| **Audit Log** | 全エージェント実行の詳細記録 |
| **Policy Manager** | 権限ポリシーを検証するコンポーネント |
| **Hot Reload** | エージェント定義の動的再読み込み |

### 8.4 参考資料

- [Claude Subagents 公式ドキュメント](https://docs.anthropic.com/claude/docs/subagents)
- [OpenAI Deep Research 発表](https://openai.com/index/deep-research/)
- [MCP 仕様（Model Context Protocol）](https://modelcontextprotocol.io/specification/latest)
- [要件定義書](docs/codex-subagents-deep-research.md)
- [実装ログ（M1）](_docs/2025-10-10_サブエージェントDeepResearch実装.md)

---

**次のアクション**: M2 実装の着手（2025-10-12 ~ 2025-10-25）

---

**文書管理**  
- **作成**: 2025-10-12 18:40 JST  
- **最終更新**: 2025-10-12 18:40 JST  
- **バージョン**: v0.1.0  
- **レビュアー**: Core Team, Deep Research Team, Supervisor Team

