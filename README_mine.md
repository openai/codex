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

## 追加機能
本家との差分機能のリスト。

- **カスタムプロンプト探索ルートの拡張**: 「リポジトリ内 `.codex/prompts`」と「`$CODEX_HOME/prompts`」を探索し、同名はリポジトリ側が優先

