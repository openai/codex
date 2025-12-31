# Changelog

この拡張はまだ初期段階です。互換性が壊れる変更が入る可能性があります。

## Unreleased

- Images: MCP image / Image view の表示を安定化（`file+.vscode-resource...` の 401 回避、`blob:` 描画 + CSP、オンデマンド読み込み/オフロード、リサイズ＆圧縮（最大辺 1024px / 目標 350KB）、`globalStorage/images.v2` キャッシュ（件数/容量上限で削除）、Webview Object URL キャッシュ上限（LRU））
- ステータス: rate limit（例: `5h:11% wk:7%`）にホバーするとリセット時刻を表示
- Interrupt: 送信直後など turnId 未確定のタイミングでも Stop/Interrupt が取りこぼされないように修正（turn/started 到達後に割り込みを送る）
- Interrupt: backend を kill/restart する Force Stop を廃止（workspace 単位で backend を共有しており他セッションも停止するため）。turnId 不明時は `thread/resume` で inProgress の turn を探索して `turn/interrupt` を試行
- Interrupt: `codexMine.interrupt.forceStopAfterMs` 設定を削除（Force Stop 廃止に伴い不要）
- Backend: backend停止時の streamState クリーンアップ不具合を修正（スレッド単位で削除）
- Backend: backend が外部要因で終了しても、セッションの `sending`/承認待ち状態が残らないように UI 状態を同期
- Agents: agents（subagents）の一覧/候補取得をローカル走査（`.codex/agents` / `$CODEX_HOME/agents`）で提供

## 0.1.12

- 拡張アイコンを追加（`resources/icon.png`）

## 0.1.11

- 入力履歴: ↑/↓ の履歴ナビゲーションが全セッション共通になっていた不具合を修正（セッションごとに独立）
- `README.md:10` / `.env.local:23` のような行番号付きファイル参照が、チャット内で開けないことがある不具合を修正（Markdownリンク / code トークン）
- パフォーマンス: 会話履歴（Runtime blocks）をワークスペースストレージにキャッシュしないように変更（履歴は `thread/resume` で `~/.codex/sessions` から復元）
- パフォーマンス: Webview の full-state 更新（`refresh`）を間引き、ストリーミング中の更新連打による Extension Host の負荷を軽減

## 0.1.10

- チャット内の `@path/to/file` 形式でも Ctrl/Cmd+Click でファイルを開けるようにした
- `openFile` の失敗ダイアログ（`No matching result`）を廃止し、`vscode.open` に委譲
- Command カード（pre/meta）内のパスも Ctrl/Cmd+Click で開けるようにした（出力が巨大な場合はリンク化を抑制）

## 0.1.9

- 承認UI: Decline/Cancel で `turn/interrupt` を送って実行中を止める（次の入力に進める）
- チャット履歴内の `http://` / `https://` を Ctrl/Cmd+Click で開けるようにした
- `@` のファイル検索を軽くした（2文字以上でのみ検索、debounce を延長、キャンセル反応を改善）

## 0.1.8

- Ctrl/Cmd+Hover 時のみファイルパスをリンク風表示（押しているだけでは表示しない）
- 実行中の停止（Esc）が入力欄フォーカス時に確実に効くよう修正

## 0.1.7

※このバージョンには、以前から実装されていたが CHANGELOG 未記載だった項目の追記も含む。

- legacy (`codex/event/*`) の表示を最小許可リストに限定し、Command/Changes 等の重複表示を抑制
  - 許可: `token_count`, `mcp_startup_update`, `mcp_startup_complete`, `turn_aborted`, `list_custom_prompts_response`
- UI: ブロックが de-dupe/削除された時に、Webview 側の残骸 DOM を掃除して重複表示を防止
- セッションをエディタタブで開けるようにした（Session Panel / `Open Session (Editor Tab)`）
- セッションメニュー追加（タブ切替/非表示/クローズ など）
- Runtime cache をワークスペース単位でクリアするコマンドを追加（`Clear Runtime Cache (Workspace)`）
- 承認（Approval）要求をチャット上にカード表示し、Accept/Decline/Cancel と「Accept (For Session)」を操作できるようにした
- `Status` で account / rate limits 等の状態を表示できるようにした
- 空のまま完了した `Reasoning` を表示しない（ノイズ削減）
- `Return to Bottom` を追加（スクロールがBottomにない時のみ表示・タブ切替時は自動でBottomへ）
- 実行中でも画像の添付/ペーストをできるようにした（次の入力に備えて溜められる）
- チャット履歴内のファイルパスを Ctrl/Cmd+Click で開けるようにした（見つからない場合は `No matching result`）
  - Ctrl/Cmd+Hover でリンク風の見た目になる
- Webview が隠れたり再生成されても、入力途中のテキストを保持するようにした（下書き保持）
- 不要な横スクロールが出ないように調整（Webview内の横方向スクロールを抑制）

## 0.1.6

- 入力欄の `@` 補完に `@agents:{name}` を追加（codex-mine実行時のみ）。ファイル候補より先に表示する

## 0.1.5

- Resume: `thread/resume` でモデルを上書きしないように修正（モデル不一致の警告を抑制）

## 0.1.4

- Resume: 履歴復元が警告/デバッグ出力に邪魔されないよう修正
- Resume: 一覧で時刻を先に表示
- デバッグ/Legacyイベントの表示を折りたたみに変更（デフォルト閉じる）

## 0.1.3

- Resume開始時に、Newと同様にディレクトリ（workspace folder）を選べるようにした（選択したディレクトリの履歴のみ表示）
- Resume の一覧から `modelProvider` / `cliVersion` の表示を削除

## 0.1.2

- ⚙ から CLI を `codex` / `codex-mine` / `auto` で切替できるようにした（以降のデフォルト。適用には backend 再起動が必要）
  - `codex-mine` 選択時は `New` が常に codex-mine backend を使うように（必要なら自動再起動）
- `codex-mine` 実行時のみ `/agents` を有効化し、`.codex/agents` / `$CODEX_HOME/agents` から選んで `@name` を入力欄へ挿入できるようにした
- app-server の `thread/list` を使って履歴一覧から `thread/resume` できる Resume 機能を追加（CHAT右上の Resume / `/resume`）
  - 履歴一覧は `CODEX_HOME` の全履歴を対象にする
- セッションをリネームした場合は、タブ/SESSIONS表示から `#N` を外す（`#N` はデフォルト名の識別用途のみ）

## 0.1.1

- upstream 準拠で `skills/list` を呼び出し、`/skills` でスキルを挿入できるようにした（repo-local `.codex/skills` も app-server 側で探索される）

## 0.1.0

- Send ボタンをアイコン化し、実行中は Stop（クリック / Esc）で `turn/interrupt` を送る
- キャンセル（legacy `turn_aborted`）をノイズ扱いせず簡易表示（`Interrupted`）
- 入力欄を 1 行ベースにして自動伸長（上に伸びる）＋高さ調整
- 右上ステータス（チェック/スピナー）の位置ずれを修正
- Output を自動で開かないように変更

## 0.0.7

- モデル一覧の取得と表示、および Reasoning effort 選択 UI を追加

## 0.0.6

- MCP startup update イベントをグローバルステータスに表示するよう改善

## 0.0.5

- shell-quote 依存が VSIX に含まれず起動時に失敗する不具合を修正

## 0.0.1

- 初期リリース（in-repo 開発版）
