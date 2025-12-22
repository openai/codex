# Changelog

この拡張はまだ初期段階です。互換性が壊れる変更が入る可能性があります。

## 0.1.0

- Send ボタンをアイコン化し、実行中は Stop（クリック / Esc）で `turn/interrupt` を送る
- キャンセル（legacy `turn_aborted`）をノイズ扱いせず簡易表示（`Interrupted`）
- 入力欄を 1 行ベースにして自動伸長（上に伸びる）＋高さ調整
- 右上ステータス（チェック/スピナー）の位置ずれを修正
- Output を自動で開かないように変更

## 0.1.1

- upstream 準拠で `skills/list` を呼び出し、`/skills` でスキルを挿入できるようにした（repo-local `.codex/skills` も app-server 側で探索される）

## 0.1.2

- ⚙ から CLI を `upstream` / `codex-mine` / `auto` で切替できるようにした（適用には backend 再起動が必要）
- `codex-mine` 実行時のみ `/agents` を有効化し、`.codex/agents` / `$CODEX_HOME/agents` から選んで `@name` を入力欄へ挿入できるようにした

## 0.1.3

- CLI 切替の表示名を `codex` / `codex-mine` に統一し、⚙ を CHAT 右上へ移動
- `cli.variant=codex-mine` のとき、`New` で必ず codex-mine backend を使うように（必要なら自動再起動）

## 0.1.4

- ⚙ の CLI 切替を「ディレクトリ選択」ではなく「以降のデフォルト」に変更（グローバル設定を更新）

## 0.1.5

- app-server の `thread/list` を使って履歴一覧から `thread/resume` できる Resume 機能を追加（CHAT右上の Resume / `/resume`）

## 0.0.7

- モデル一覧の取得と表示、および Reasoning effort 選択 UI を追加

## 0.0.6

- MCP startup update イベントをグローバルステータスに表示するよう改善

## 0.0.5

- shell-quote 依存が VSIX に含まれず起動時に失敗する不具合を修正

## 0.0.1

- 初期リリース（in-repo 開発版）
