
# zapabob/codex — Sub‑Agents & DeepResearch 拡張 要件定義書（v0.1, 2025‑11‑01, JST）

## 1. 背景・現状認識（コードベースレビュー）
- **ベース**は `openai/codex` の最新メインブランチ（0.53.0, Oct 31, 2025）。Rust中心（約96%）で CLI (`codex-cli`) とコア (`codex-rs`)、Docs を含む。MCP（Model Context Protocol）対応、承認/サンドボックス、AGENTS.md（メモリ）等のドキュメントが整備されている。〔参照: README, Languages, MCP, Sandbox & approvals〕
- zapabob による **上流PR活動**の痕跡（meta‑orchestration/parallel agent 等）あり。〔参照: GitHub Actions の run logs / PR #5108 連動〕
- 競合プロダクト：**Claude Code**のサブエージェント（専用プロンプト＋ツール＋独立コンテキスト）、**Gemini CLI**（オープンソース CLI、MCP/検索連携、GitHub Actions 統合の報道/OSS）。これらは**研究/検索のネイティブ統合**や**エージェント分業**を強化している。

> 本要件は **openai/codex の構造とAPIに追従**しつつ、zapabob/codex 側で **サブエージェント群** と **DeepResearch（検索拡張）** を追加する前提。

---

## 2. 目的・非目的
**目的**
1) Codex に **タスク特化サブエージェント**をネイティブ実装（並列実行＋合議統合）。  
2) MCP ベースの **DeepResearch**（検索→要約→根拠スコア→引用出力）を公式検索機能の拡張として提供。  
3) **安全性・再現性**（承認フロー、監査ログ、決定記録）と **Git 連携**（worktree 競合回避/選抜）を強化。

**非目的**
- LLM 推論エンジン自体の改変（モデル切替は設定で）。
- IDE プラグインの再実装（CLI から利用可能な範囲に限定）。

---

## 3. 機能要件（FR）

### FR-1 サブエージェント基盤
- 役割プリセット：`researcher`, `architect`, `executor`, `tester`, `sec-auditor`, `refactorer`。  
- **独立コンテキスト**（各自メモリ/Scratchpad）と **専用ツール権限**（MCP/FS/Net）をプロファイルで定義。  
- 並列実行（N 並列）＋**編集範囲のシャーディング**（glob/pattern）。  
- **合議統合**：スコアリング（テスト/静的解析/依存差分/ベンチ/可読性）で最良案を採択。

### FR-2 DeepResearch（検索拡張）
- MCP サーチアダプタ（例：Google/Brave/Serper/Tavily 等の抽象化）＋ **去重（near-dup）**、**ソース信頼度**、**日付整合性**。  
- **evidence JSON**（`title`,`url`,`published`,`quote`,`confidence`）生成と **出力へのインライン引用**。  
- **再現用 seed/query-log** を `./.codex/research/` に保存。

### FR-3 Git/ワークスペース戦略
- `--worktree-competition` モード：各サブエージェントが独立 worktree/ブランチで実装→**自動スコアで勝者 PR**。  
- `--orchestrated-edit` モード：メインが **intent-to-edit ロック**（ファイル粒度）を発行し直列化。  
- マージキュー（CI 緑のみ自動取込み）。

### FR-4 承認・サンドボックス・監査
- 既存の **Sandbox & approvals** に連結：書込み/ネット/プロセス権限は **ポリシー TOML** で明示。  
- すべての自動コミット/PRに **Decision Log**（理由・スコア・根拠URL）を付与。

### FR-5 観測性
- 構造化ログ（JSONL）と **OpenTelemetry** 互換の trace 出力。  
- 失敗時の**リトライ方針**（指数バックオフ＋代替サブエージェント）。

---

## 4. 非機能要件（NFR）
- **性能**：サブエージェント並列 N=3 で 90p タスクを 30% 短縮（ベンチ用テンプレを提供）。  
- **コスト**：トークン予算上限（例：15k/request、総 200k/job）を `budgeter` で強制。  
- **セキュリティ**：デフォルトは読み取り中心、書込みは承認必須。ZDR/プロキシ経由も選択可。  
- **再現性**：研究結果とビルドを `codex.lock` + `research/*.json` でピン留め。

---

## 5. CLI / 設定仕様（案）
### 主要コマンド
- `codex plan --goal "<text>" --agents researcher,architect,executor --parallel 3`
- `codex research "<query>" --timebox 180s --sources web,mcp --evidence out.json`
- `codex run --worktree-competition --score-profile default`
- `codex approve --pr <id> --policy strict`
- `codex exec --playbook refactor/api --orchestrated-edit`

### 設定ファイル（`~/.codex/config.toml` 抜粋）
```toml
[agents.researcher]
tools = ["mcp.search", "web.scraper"]
context_tokens = 60000

[git]
mode = "worktree-competition" # or "orchestrated-edit"
merge_queue = true

[budgeter]
per_request = 15000
per_job = 200000
```

---

## 6. アーキテクチャ（変更点）
- **codex-rs**：`orchestrator/` に `subagent_runtime.rs`, `scoring.rs`, `locks.rs`, `deepresearch/` を追加。  
- **codex-cli**：新規サブコマンド群（plan/research/run/approve）。  
- **docs**：`SUBAGENTS.md`, `DEEPRESEARCH.md`, `WORKTREES.md`。

---

## 7. スコアリング指標（デフォルト）
1) テスト: pass率/coverageΔ  
2) 静的解析: lint/type/sec（CVE差分）  
3) パフォーマンス: microbenchΔ/ビルド時間Δ  
4) 変更リスク: 変更ファイル数/公開API変更/行数  
5) 可読性: formatter適合/循環依存Δ

---

## 8. 移行・ロールアウト
- M0: PoC（単一 repo、N=2） → M1: 小規模 OSS → M2: 本番（merge queue＋監査ログ必須）。  
- Gate 基準：AC テスト（下記）とセキュリティレビュー通過。

---

## 9. 受け入れ基準（AC）
- `codex research` が **日付順ソート＋重複排除**を行い、**引用付き要約**を生成。  
- `--worktree-competition` で **3案から1案を自動 PR 化**、Decision Log に根拠を記録。  
- 競合改修で **intent ロック**によりファイル競合ゼロ（手動介入なし）。

---

## 10. リスクと緩和
- **検索APIの変動** → MCP アダプタで抽象化＋フォールバック複数実装。  
- **コスト肥大** → budgeter/early‑stop/要約縮約。  
- **誤統合** → 厳格スコア閾値＋人手承認＋ロールバック自動化。

---

## 参考（比較資料）
- Claude Code サブエージェント（独立コンテキスト/ツール/プロンプト）。
- Gemini CLI（OSS、MCP/検索/Actions連携の公開情報）。
- openai/codex README（MCP/承認/CLI/リリース/言語構成）。
