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
| `config.toml` | `cwd/.codex/config.toml` / `$CODEX_HOME/config.toml` | **`cwd/.codex/config.toml` が存在する場合、`$CODEX_HOME/config.toml` は読み込まない**（Codex-Mine方針）。その上に managed config / CLI overrides が乗る。 | 手編集 | dotenv は `cwd/.codex/.env` があればそれのみ読み込み、無ければ `$CODEX_HOME/.env` を読む（dotenv から `CODEX_` は読まない）。 |
| `mcp_servers`（MCP） | `cwd/.codex/config.toml` / `$CODEX_HOME/config.toml` | cwd-local `config.toml` がある場合は **global 側が読み込まれない**ため、cwd-local の `mcp_servers` のみが有効。 | `codex mcp add/remove` は **常に** `$CODEX_HOME/config.toml` を更新。cwd-local は手編集。 | `codex mcp` は cwd-local には書かない（`-g/--global` フラグもない）。cwd-local `config.toml` がある場合、`codex mcp add/remove` の変更はその作業ディレクトリでは反映されない。 |
| `prompts` | `repo/.codex/prompts/` / `$CODEX_HOME/prompts/` | `<git root>/.codex/prompts` → `$CODEX_HOME/prompts` の順に探索し、同名は repo 側が優先。 | 追加/編集/削除（`.md`） | `.md` のみ対象。 |
| `skills` | `repo/.codex/skills/` / `$CODEX_HOME/skills/` | **repo-local からの読み込みに対応済み**。git repo 内では `cwd` から repo root までの間で最初に見つかった `.codex/skills` を優先し、次に `$CODEX_HOME/skills`（→ system → admin）。 | 追加/編集/削除（`SKILL.md`） | 同名 skill は repo が優先で dedupe。 |
| `agents`（subagents） | `<git root>/.codex/agents/` / `$CODEX_HOME/agents/` | `<git root>/.codex/agents` → `$CODEX_HOME/agents` の順。 | 追加/編集/削除（`<name>.md`） | skills と違い、subagents は「最寄り `.codex`」探索はしない。 |

### subagents

- `@name <prompt>` 形式の指示を解釈してサブエージェント実行を補助（`.codex/agents` / `$CODEX_HOME/agents` から定義を探索）
- `run_subagent` 実行時に **親ターンのキャンセルが伝搬**（Ctrl+C / TurnAborted 等でサブエージェントも止まる）

### hooks

Codex-Mine では `config.toml` に `[[hooks]]` を定義して、内部イベント（例: `turn.end`, `web_search.end`, `tool.exec.begin/end`, `tool.call.begin/end`）に応じて外部コマンドを起動できる。

- 設定場所: `cwd/.codex/config.toml`（存在する場合は `$CODEX_HOME/config.toml` は読み込まれないため、hooks も「マージ」されず二重発火しない）
- スクリプト置き場例: `cwd/.codex/hooks/*.py`（音を鳴らす等の確認用サンプルを置ける）
- 特徴: observe-only（失敗はログに出るが、エージェントの実行は止めない）
- 実行cwd: 可能なら git repo root、無ければセッションの `cwd`

#### 最小例

```toml
[[hooks]]
id = "log-turn-end"
when = "turn.end"
command = ["python3", ".codex/hooks/log_event.py", ".memo/logs/hooks/turn.end.log"]
timeout_ms = 2000
```

#### フィールド

- `id`（任意）: ログ用の識別子。未指定でも動く。
- `when`（必須）: 1つまたは複数のイベント種別。
  - 例: `when = "tool.call.end"` / `when = ["tool.exec.begin", "tool.exec.end"]`
- `command`（必須）: 起動する外部コマンド（argv配列）。hook の入力JSONは **stdin** に渡される。
- `timeout_ms`（任意）: hook コマンドのタイムアウト（ミリ秒）。
- `matcher`（任意）: イベントに応じてマッチ対象が変わる正規表現（Rust `regex`）。
  - `tool.call.*`: `tool_name`（例: `apply_patch`, `mcp__chrome-devtools__list_pages`, `exec_command`）
  - `tool.mcp.*`: MCP tool 名（例: `list_pages`）
  - `tool.exec.*`: 実行ソース（`shell` / `unified_exec` / `user_shell`）
  - それ以外: 現状マッチ対象なし（`matcher` 指定しても絞れない）

#### `matcher` の例（種類別にHooksを仕込む）

```toml
# chrome-devtools MCP だけ（tool.call.* で qualified name を絞る）
[[hooks]]
when = ["tool.call.begin", "tool.call.end"]
matcher = "^mcp__chrome-devtools__"
command = ["python3", ".codex/hooks/log_event.py", ".memo/logs/hooks/mcp.chrome.tool-call.log"]

# MCP の list_pages だけ（tool.mcp.* で tool 名を絞る）
[[hooks]]
when = ["tool.mcp.begin", "tool.mcp.end"]
matcher = "^list_pages$"
command = ["python3", ".codex/hooks/log_event.py", ".memo/logs/hooks/mcp.list_pages.log"]

# unified_exec のみ（tool.exec.* で exec 種別を絞る）
[[hooks]]
when = ["tool.exec.begin", "tool.exec.end"]
matcher = "^unified_exec$"
command = ["python3", ".codex/hooks/log_event.py", ".memo/logs/hooks/exec.unified.log"]
```

#### payload（stdin JSON）の見方

最低限、`type`（イベント種別）と `thread_id` / `turn_id` が入る。ツール系は `tool_name` / `call_id` が入る。

- 例: `tool.call.begin` / `tool.call.end`
  - `type`: `"tool.call.begin"` / `"tool.call.end"`
  - `tool_name`: 例 `mcp__chrome-devtools__list_pages`
  - `call_id`: 例 `call_...`

詳細を見たい場合は `dump_payload.py` のように stdin を jsonl で保存して確認する。

#### デバッグ

hooks が発火しているかは `codex_core::hooks` の debug ログで追える。

```sh
RUST_LOG=codex_core::hooks=debug codex-mine exec --json '...'
```
