# codex-mine

このリポジトリをローカルでビルドした Rust 版 Codex を、npm で入れている `codex` と衝突させずに並行運用するためのメモ。

## インストール/更新

リポジトリ直下で実行:

- `./scripts/install-codex-mine.sh`

## 起動コマンド

- npm 版: `codex`
- ローカルビルド版: `codex-mine`
  - 実体は `~/.local/codex-mine/bin/codex`
  - `~/.local/bin/codex-mine` はラッパーで、起動時に `--config check_for_update_on_startup=false` を付けて「Update available!」の通知を無効化する

## Upstreamとの差分（主なもの）
`upstream/main`（openai/codex）に対する `codex-mine` の差分のうち、運用上影響が大きいもの。

### repo-local `.codex/` 運用（#1, #6, #8, #9）

- **優先順位**: git repo 内では `repo/.codex/config.toml` が `$CODEX_HOME/config.toml` より優先（詳細: `codex-rs/core/src/config_loader/README.md`）。dotenv は `$CODEX_HOME/.env` → `repo/.codex/.env` の順（dotenv から `CODEX_` は読まない）。
- **置き場所**: `repo/.codex/config.toml`（設定/MCP）、`repo/.codex/.env`（シークレット・非コミット）、`repo/.codex/prompts/`（saved prompts）、`repo/.codex/agents/`（subagents）。
- **prompts の探索/マージ**: `repo/.codex/prompts/` と `$CODEX_HOME/prompts` を両方探索し、同名は repo 側が優先。
- **MCP 設定**: repo 内の `codex mcp add/remove` は `repo/.codex/config.toml` を更新（`-g/--global` で `$CODEX_HOME`）。repo 側に `mcp_servers` があれば global 側は「置換」。

### subagents（#6, #9）
- `@name <prompt>` 形式の指示を解釈してサブエージェント実行を補助（`.codex/agents` / `$CODEX_HOME/agents` から定義を探索）
- `run_subagent` 実行時に **親ターンのキャンセルが伝搬**（Ctrl+C / TurnAborted 等でサブエージェントも止まる）
