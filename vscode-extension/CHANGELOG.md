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

- ⚙ から CLI を `codex` / `codex-mine` / `auto` で切替できるようにした（以降のデフォルト。適用には backend 再起動が必要）
  - `codex-mine` 選択時は `New` が常に codex-mine backend を使うように（必要なら自動再起動）
- `codex-mine` 実行時のみ `/agents` を有効化し、`.codex/agents` / `$CODEX_HOME/agents` から選んで `@name` を入力欄へ挿入できるようにした
- app-server の `thread/list` を使って履歴一覧から `thread/resume` できる Resume 機能を追加（CHAT右上の Resume / `/resume`）
  - 履歴一覧は `CODEX_HOME` の全履歴を対象にする
- セッションをリネームした場合は、タブ/SESSIONS表示から `#N` を外す（`#N` はデフォルト名の識別用途のみ）

## 0.1.3

- Resume開始時に、Newと同様にディレクトリ（workspace folder）を選べるようにした（選択したディレクトリの履歴のみ表示）
- Resume の一覧から `modelProvider` / `cliVersion` の表示を削除

## 0.1.4

- Resume: 履歴復元が警告/デバッグ出力に邪魔されないよう修正
- Resume: 一覧で時刻を先に表示
- デバッグ/Legacyイベントの表示を折りたたみに変更（デフォルト閉じる）

## 0.1.5

- Resume: `thread/resume` でモデルを上書きしないように修正（モデル不一致の警告を抑制）

## 0.0.7

- モデル一覧の取得と表示、および Reasoning effort 選択 UI を追加

## 0.0.6

- MCP startup update イベントをグローバルステータスに表示するよう改善

## 0.0.5

- shell-quote 依存が VSIX に含まれず起動時に失敗する不具合を修正

## 0.0.1

- 初期リリース（in-repo 開発版）
