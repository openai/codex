# Codex サブエージェント & Deep Research 実装計画 (v1)

**ステータス:** Draft · **対象読者:** Codex core / orchestrator, CLI・IDE・Web・Slack・GitHub 各面の実装担当, zapabob フォーク保守チーム, セキュリティ & SRE

---

## 0. エグゼクティブサマリ
- サブエージェント Runtime と Deep Research Engine を Codex 中核へ組み込むための 4 フェーズ (M1〜M4) ロードマップを定義。zapabob フォークを用いた段階導入→upstream 反映の手順を明文化。
- 主要ワークストリーム (Runtime/Orchestrator, Budgeter・Policy Guard, Deep Research 計画エンジン, MCP 連携, UI/CLI/IDE/Slack/GitHub サーフェス, 観測性 & コスト, QA & ガバナンス) ごとに成果物と依存関係を提示。
- 非機能要件 (サンドボックス権限ガード, トークン予算管理, 出典トラッキング, 監査ログ) を各フェーズの出口基準へ反映。Lite/Fallback 経路を標準ルートへ組み込み。
- ローンチゲート (パフォーマンス KPI, セキュリティレビュー, 運用 Runbook, Partner Beta 成果) を設定し、GA 移行時のメトリクスと残課題を明文化。

---

## 1. 目的とガイドライン
1. **ガイデッド・デリゲーション:** `.codex/agents/*.yaml|md` に基づく権限・トークン境界を維持しつつ、Codex 既存動線から破壊的変更なく呼び出せるサブエージェント体験を提供する。
2. **計画型検索統合:** Deep Research のサブクエリ計画／多段探索／引用必須レポートの流れを Codex の標準検索として再設計し、Lite モードを含むフェイルオーバを内包する。
3. **二系統保守:** `openai/codex` と `zapabob/codex` の双方向同期を前提に、Capability Toggle と Feature Flag を用いて差分を吸収する。
4. **コスト透明性:** Budgeter, Lite DR, 出典ログ, MCP 権限審査を通じ、長時間タスクでも予算逸脱・セキュリティ逸脱を防ぎつつ監査可能性を担保する。

---

## 2. ターゲットアーキテクチャ概要

### 2.1 サブエージェント Runtime
- **Coordinator (codex-core)**: デリゲーション要求を評価し、Agent Manifest の Capability/Policy を確認。Budgeter からトークン配分を取得し、実行ノードを起動。
- **Agent Sandboxes**: 各サブエージェントごとに会話バッファ、エフェメラルな git worktree、Telemetry Channel を分離。Seatbelt / CODEX_SANDBOX 環境変数を尊重し、権限は Manifest の allowlist で制御。
- **Result Broker**: Artifact (diff, report, metrics) を構造化してメインエージェントへ返却。並列タスクはワークフロー ID を持ち、合流時に衝突検出と PR Sharding を補助。

### 2.2 Deep Research Engine
- **Planner**: トピックからサブクエリ列・評価基準・Stopping 条件 (引用閾値/反証プローブ) を生成。Lite モード用の縮退プランも同時生成。
- **Explorer**: MCP Discovery で取得したツールを Sandbox 実行し、検索→抽出→統合→矛盾検出を繰り返す。Retries 時は Budgeter へコスト請求。
- **Synthesizer**: finding ごとに引用・証拠を付与し、Codex surfaces 向けの表示形式 (CLI 表, Web/IDE Markdown, Slack Block Kit) を生成。

### 2.3 クロスカット
- **Budgeter**: エージェント別トークン残高、最大実行時間、MCP ツールコストを追跡。閾値を超えると Runtime へ中断・Lite モード移行を指示。
- **Observability**: OpenTelemetry ベースの spans/events を発行し、出典 ID・ワークフロー ID・Surface ID をタグ付け。監査ログは永続ストレージへ 30 日以上保持。
- **Feature Flags**: サービス種別 (CLI, IDE, Slack, GitHub, Web) とデプロイ系統 (openai / zapabob) ごとに Runtime Capability を切り替える。

---

## 3. ワークストリーム別計画

### 3.1 Runtime & Orchestrator
- Agent Manifest ローダー、Policy Validator、Budgeter API、Delegation Graph 実行器、Result Broker を段階実装。
- Git worktree 分離・PR Branch 命名規約・CI 自動キューを codex-core へ追加。
- Telemetry Hook とイベントスキーマを定義 (`agent_started`, `budget_adjusted`, `artifact_ready` など)。
- ClaudeCode 風オーケストレーターにタスクレジストリとイベントログを常設し、並列実行時の衝突検知とサーフェス横断の進行把握に利用。

### 3.2 Budgeter / Policy Guard
- 動的トークン配分アルゴリズム (初期: static quota + burst, 後続: feedback-based)。
- Shell / MCP / Network 権限用の Policy DSL を整備し、Seatbelt 互換の Deny-First 評価を追加。
- Lite Deep Research のフォールバック条件 (Budget < X, Tool Failure, Network Deny) を定義し、Runtime へレスポンス。

### 3.3 Deep Research Engine
- Planner モジュール (prompt templates, scoring heuristics, evaluation rubrics)。
- Explorer タスクランナー (再入可能ジョブ, MCP ツール監視, 出典抽出 pipeline)。
- Synthesizer (引用レンダリング, 反証結果表示, Lite/Lite+ モード対応)。
- フェイルオーバのための Observability ハンドラ (error_class, fallback_reason)。

### 3.4 MCP & External Tooling
- MCP Discovery キャッシュ、Scoped Credential Vault、Tool Allowlist UI。
- CORS / 認証付きエンドポイントのプロキシング、Seatbelt 下での軽量 HTTP クライアント整備。
- ランタイム互換テスト (zapabob fork + upstream) を CI に追加。

### 3.5 Surfaces (CLI, IDE, Web, Slack, GitHub)
- 共通 GraphQL/HTTP API の `delegate` / `research` エンドポイント拡張。
- CLI: `codex delegate`, `codex research` コマンド刷新 (progress streaming, artifact pull)。
- IDE: エージェント選択 UI、Research Report パネル、Branch Preview。
- Web: Research Dashboard、並列サブエージェントのステータス表示、Citation ビューア。
- Slack/GitHub: Short-lived links で Artifact を共有し、必要に応じて手動承認フロー。

### 3.6 Observability, Cost & Audit
- OTel Collector と SLO ダッシュボード (成功率、平均トークン消費、Fallback 発生率)。
- 監査ログ (誰がどのサブエージェント／ツールを利用したか) を永続化し、Zapier / SIEM 連携。
- KPI: Delegation latency < 5s (p95), Deep Research completion < 12 min (p95), Citation coverage 100%。

### 3.7 QA, Compliance, Enablement
- サンドボックス回帰テスト、Policy バイパス検証、Seatbelt 下でのエンドツーエンド。
- Partner Beta (zapabob) 用フィードバックサイクルと issue templating。
- ドキュメント (開発者ガイド, Runbook, Incident Playbook) とトレーニングセッション。

---

## 4. マイルストーンとフェーズ

### M0: Foundation Intake (2 週間) — 任意だが推奨
- Agent Manifest 仕様凍結、Budgeter API スケッチ、Telemetry Schema 初版。
- CI に Codex sandbox 変数と Lite DR のスキップ条件を追加。
- Exit: 主要ユースケース (ライブラリ刷新 / リサーチ / セキュリティ) のユーザーストーリー承認。

### M1: サブエージェント MVP (4〜6 週間)
- Runtime: Manifest ロード, Delegation API (CLI/SDK), Worktree 分離 (単一 repo)。
- Budgeter: static quota + burst, Policy Guard v1 (shell/network allowlist)。
- Surfaces: CLI/IDE プレビュー, Slack DM 通知, GitHub コメント連携は手動トリガ。
- Observability: 基本メトリクス (起動/完了/失敗カウント), Audit ログ α。
- Exit 条件: 代表ユースケース 3 件が CLI / IDE で end-to-end 完走、Policy 逸脱ゼロ。

### M2: Deep Research v1 (5〜7 週間)
- Planner/Explorer/Synthesizer v1 を codex-deep-research crate に統合。
- MCP Discovery integration、Lite fallback、Citation 検証ハンドラ。
- Surfaces: `codex research` GA, Web Research Console β, Slack/GitHub でレポート共有リンク。
- Observability: 出典追跡, fallback_reason 集計, Budget 逸脱アラート。
- Exit 条件: Research 成功率 ≥ 85%, Citation 欠落率 < 2%, Lite fallback で成功したケース記録。

### M3: 統合 & ガバナンス (4〜5 週間)
- Multi-tenant / multi-repo Delegation, PR Sharding + Auto CI, Runtime 再試行戦略。
- Policy Guard v2 (MCP Scope, Secrets 管理, Seatbelt 互換), Budgeter フィードバック制御。
- Surfaces: Web/IDE/Slack/GitHub 全面統合 + Feature Flag, Upstream/downstream 同期。
- Observability: OTel Exporter GA, Runbook 完成, Partner Beta (zapabob) 報告。
- Exit 条件: Dual-maintenance フラグが openai/zapabob で同一挙動, Policy 監査合格, Runbook 承認。

### M4: GA & 拡張 (3〜4 週間)
- スケール試験 (50+ 並列サブエージェント, 10+ 同時 Research) と SLO 調整。
- Cost guardrail (予算アラート, 月次サマリ), SLA ドキュメント, Customer Comms。
- Backlog: Advanced personas, Auto remediation, 自動 PR merge ガイドライン。
- Exit 条件: KPI 達成, Customer SLA サインオフ, 残課題リスト公表, Post-launch Monitoring 体制確立。

---

## 5. 開発プロセスと QA
- **Branch 策定:** zapabob で feature branch → nightly merge to openai/codex via FF → GA 前に長期サポートブランチを切る。
- **CI パイプライン:** lint/test (`just fmt`, `just fix -p`, targeted cargo test), Seatbelt e2e, Lite fallback regression, MCP mock server。
- **テストデータ:** 大規模 repo モック, 公開ウェブ検索, 内部 MCP ツール (検索, PDF, カレンダー) のスタブ。
- **Release Train:** bi-weekly Beta drop (zapabob), monthly upstream sync, GA 時に Feature Flags を openai 側で有効化。

---

## 6. サーフェス別統合詳細
- **CLI:** Progress streaming (ratatui), artifact pull (`codex delegate --artifact <id>`), Budgeter 警告のリアルタイム表示。
- **IDE (VS Code / Cursor):** Sub-agent picker, Research summary panel, Inline annotation for citations, Background branch diff viewer。
- **Web Console:** Delegation board (Swimlane view), Research timeline (plan vs actual), Lite fallback notificationバナー。
- **Slack:** App Home に進行状況, DM で artifact preview, Approval workflow (deploy, merge)。
- **GitHub:** PR Check Suite 結果に Sub-agent activity を記録, Research report を PR コメントに添付, Auto create `codex/<agent>/<task>` branches。

---

## 7. セキュリティ, コンプライアンス, ガバナンス
- **Policy Authoring:** Manifest DSL with `capabilities`, `deny`, `quota`, `mcp_scopes`, `sandbox_mode`。Validation CLI で PR 時に自動検証。
- **Secrets Handling:** Agent ごとの scoped token vault (HashiCorp Vault 互換 API), access は Budgeter を経由。
- **Audit:** すべてのデリゲーションに `actor`, `agent_id`, `tool`, `cost`, `outcome`, `citation` を記録し、SIEM へ 15 分以内に転送。
- **Compliance Reviews:** Privacy/Data, Security, Accessibility のトラックを M2/M3 フェーズ内で完了し、GA 前に sign-off を得る。

---

## 8. 観測性 & コスト管理
- **Metrics:** Delegation latency, Success rate, Token per task, MCP call count, Fallback usage, Citation completeness。
- **Tracing:** Planner/Explorer/Synthesizer それぞれに span を発行し、失敗時は `error.type` を分類 (tool_denied, quota_exceeded, contradiction)。
- **Budget Dashboards:** Surfacing daily/月次コスト、Agent 別 burn rate、Lite fallback トリガ分析。
- **Alerting:** p95 latency > 10s, Success rate < 80%, Token burn rate > SLA, Audit pipeline failure を PagerDuty 連携。

---

## 9. ローンチ戦略 & KPI
- **ローンチフラグ:** `subagents_runtime`, `deep_research_planner`, `mcp_autodiscovery`, `lite_fallback`, `audit_stream`。
- **パイロット:** zapabob パートナー (内部開発者 3 チーム + 選抜 OSS) で 4 週間。Success 指標: NPS > 30, 手動介入率 < 15%。
- **GA KPI:** Delegation success ≥ 90%, Research report citation coverage 100%, コスト逸脱 0 件, セキュリティインシデント 0。
- **Post-GA:** Quarterly roadmap review, advanced persona builder, marketplace 連携検討。

---

## 10. リスクと緩和策
- **権限逸脱:** Policy Validator + Seatbelt 強制 + Audit モニタリング。フェーズごとに手動ペネテスト。
- **コスト暴走:** Budgeter hard limit + Lite fallback + Slack/Email alerts。試験的に auto-stop threshold を β 運用。
- **MCP 依存障害:** Tool health check + circuit breaker + オフラインキャッシュ。Lite 感度調整で graceful degradation。
- **Upstream Divergence:** Feature Flag で差分吸収 + contract tests + monthly merge committee。
- **ユーザー体験複雑化:** UX Research フィードバックループ + ハンドブック + tooltips/inline help を surfaces に追加。

---

## 11. 未確定事項と次アクション
1. Planner の評価指標 (LLM-based scoring vs heuristic) をどのモデルで運用するか要決定 (M1 末まで)。
2. Budgeter の persistent store (Redis vs Postgres) 選定を SRE と協議 (M0 内)。
3. Audit ログの保管期間・暗号化要件を Security/Compliance から確定 (M2 まで)。
4. Slack/GitHub 向け artifact 配信における一時認証 URL の TTL/認証方式を Legal チームと擦り合わせ (M2)。
5. zapabob ↔ openai 間の release window を Ops と調整し、Backport プロセスを文書化 (M1 前)。
6. Seatbelt 下での MCP HTTP プロキシングの性能検証と最適化案 (M1 中に PoC)。

---

**変更履歴**
- 2025-10-12: 文書初版 (Sub-Agent & Deep Research 統合計画)。作者: Codex Implementation Lead。
