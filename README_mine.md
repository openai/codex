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

### バージョニング方針

- CLI の表示バージョンは upstream の Rust リリースタグをベースにし、末尾に `-mine.x` を付ける（例: `0.76.0-alpha.8-mine.0`）。  
- 「どの upstream に基づくか」を一目で分かるようにするためで、機能差分を示す独自番号は `mine.*` で刻む。  
- crates.io には publish しない前提のローカル版想定。

### repo-local `.codex/` 運用

git repo 内では、repo-local の `.codex/` を優先して読み込むものがある。
（`config.toml` の正確なレイヤ順は `codex-rs/core/src/config_loader/README.md` を参照）

| 対象 | 置き場所 | 読み込み/優先順位 | 更新方法 | 備考 |
| --- | --- | --- | --- | --- |
| `config.toml` | `repo/.codex/config.toml` / `$CODEX_HOME/config.toml` | **repo-local が優先**（git repo 内のみ）。その上に managed config / CLI overrides が乗る。 | 手編集 | dotenv は `$CODEX_HOME/.env` → `repo/.codex/.env` の順（dotenv から `CODEX_` は読まない）。 |
| `mcp_servers`（MCP） | `repo/.codex/config.toml` / `$CODEX_HOME/config.toml` | **repo-local が優先**（layering の結果、global 側の `mcp_servers` は実質「置換」）。 | `codex mcp add/remove` は **常に** `$CODEX_HOME/config.toml` を更新。repo-local は手編集。 | `codex mcp` は git repo 内でも repo-local には書かない（`-g/--global` フラグもない）。 |
| `prompts` | `repo/.codex/prompts/` / `$CODEX_HOME/prompts/` | `<git root>/.codex/prompts` → `$CODEX_HOME/prompts` の順に探索し、同名は repo 側が優先。 | 追加/編集/削除（`.md`） | `.md` のみ対象。 |
| `skills` | `repo/.codex/skills/` / `$CODEX_HOME/skills/` | **repo-local からの読み込みに対応済み**。git repo 内では `cwd` から repo root までの間で最初に見つかった `.codex/skills` を優先し、次に `$CODEX_HOME/skills`（→ system → admin）。 | 追加/編集/削除（`SKILL.md`） | 同名 skill は repo が優先で dedupe。 |
| `agents`（subagents） | `<git root>/.codex/agents/` / `$CODEX_HOME/agents/` | `<git root>/.codex/agents` → `$CODEX_HOME/agents` の順。 | 追加/編集/削除（`<name>.md`） | skills と違い、subagents は「最寄り `.codex`」探索はしない。 |

### subagents

- `@name <prompt>` 形式の指示を解釈してサブエージェント実行を補助（`.codex/agents` / `$CODEX_HOME/agents` から定義を探索）
- `run_subagent` 実行時に **親ターンのキャンセルが伝搬**（Ctrl+C / TurnAborted 等でサブエージェントも止まる）
