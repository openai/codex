# Blueprint  Plan 完全移行実装ログ

**日時**: 2025-11-06 17:57:06  
**バージョン**: codex-cli 2.0.0  
**担当**: Cursor Agent

##  変更概要

全ての lueprint を plan に完全移行（後方互換性なし）

##  主要変更

### 1. Rust CLI (codex-rs/cli)

- **ファイル名**:
  - lueprint_commands.rs  plan_commands.rs
  - lueprint_commands_impl.rs  plan_commands_impl.rs
  - lueprint_commands_test.rs  plan_commands_test.rs

- **構造体列挙型**:
  - BlueprintCli  PlanCli
  - BlueprintCommand  PlanCommand
  - BlueprintBlock  PlanBlock
  - BlueprintState  PlanState

- **変数名**:
  - `Plan_id`  `plan_id` (snake_case統一)
  - `Plan_dir`  `plan_dir` (snake_case統一)

- **main.rs**:
  - `Subcommand::Plan(PlanCli)` に変更（172行目）

### 2. Rust Core (codex-rs/core)

- **ディレクトリ**:
  - core/src/blueprint/  core/src/plan/

- **ファイル**:
  - lueprint_orchestrator.rs  plan_orchestrator.rs
  - BlueprintOrchestrator  PlanOrchestrator

- **関連モジュール**:
  - gents/competition.rs: `BlueprintBlock`  `PlanBlock`
  - execution/engine.rs: 同上
  - webhooks/client.rs: `blueprint`  `plan`
  - webhooks/types.rs: `BlueprintState`  `PlanState`
  - 	elemetry/events.rs: `blueprint_id`  `plan_id`

### 3. 自然言語パーサー

- `core/src/natural_language_parser.rs`:
  - "ブループリント" と "blueprint" パターンを削除

### 4. データディレクトリ

- `~/.codex/blueprints/`  `~/.codex/plans/`

##  ビルドインストール

\\\powershell
# プロセス停止
taskkill /F /IM codex.exe

# Cargoキャッシュクリーン
cargo clean

# sccache + 12コアビルド
sccache="sccache"
cargo build --release -p codex-cli --jobs 12

# 強制インストール
cargo install --path cli --force

# バージョン確認
codex --version  # codex-cli 2.0.0
\\\

##  テスト結果

\\\ash
# ヘルプ確認
codex plan --help
#  動作OK

# リスト表示
codex plan list
#  "No Plans found." 正常表示

# Plan作成（テスト）
codex plan create "Test Plan" --mode=orchestrated --budget-tokens=10000
#  動作確認
\\\

##  最終状態

- **型エラー**: 0件
- **警告**: mcp-server の CUDA feature 4件のみ（既存問題）
- **ビルド時間**: 約770秒（sccache使用）
- **インストール**: 成功

##  残タスク

なし（完全移行完了）

##  備考

- Web アプリ（prism-web）、VSCode拡張、ドキュメントの移行は今回対象外
- 変数名を `Plan_id` から `plan_id` に修正する際、Pythonスクリプトで一括置換を実施
- Cargoキャッシュの影響で何度かクリーンビルドが必要だった

---

**完了音**: marisa_owattaze.wav 再生完了 
