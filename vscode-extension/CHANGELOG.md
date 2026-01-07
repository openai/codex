# Changelog

この拡張はまだ初期段階です。互換性が壊れる変更が入る可能性があります。

## Unreleased

- （なし）

## 0.1.15

- 改善: チャット内のファイルパスリンク化で日本語（Unicode）をサポート
  - 通常テキスト: 全角スペース（`　`）と `・` を含むパスをリンク化（半角スペースは区切りとして扱う）
  - コードブロック: 半角スペースを含むパスもリンク化
- 変更: codex backend 利用時は、未対応の機能（Rewind/Edit, /compact, Reload）を UI から無効化
- 改善: 上部のボタンを整理（`Status` / `Open Latest Diff` を削除し、`/status` / `/diff` から実行）

## 0.1.14

- 追加: セッションの `reload` / `rewind` / `undo` をサポート
- 変更: 過去 turn の Edit（rewind）は会話のみを巻き戻す（ファイル/作業ツリーは巻き戻らない。Issue #23）
- 追加: `/compact`（サーバー側の `thread/compact`）の実行状況を UI で見える化（Contextカードを即時表示、進行中はスピナー、完了はチェック。失敗は×＋エラー）
- 改善: 大量の `delta` 受信時に UI 更新が詰まって止まるケースを軽減（更新の coalesce、差分追記の効率化）
- 改善: `thread/resume` 時に `cwd` / model 等を上書きしない（進行中 turn のストリームを壊しにくくする。loaded conversation は fast-path で復元）
- 修正: セッション切替（`selectSession`）で裏の `thread/resume` を実行しない（進行中 turn のストリーム競合を回避）
- 修正: `@` のファイル検索候補が更新されずに詰まることがある
- 変更: Interrupt の挙動を見直し（turnId 不明時に `thread/resume` で inProgress turn を探索しない。`turn/started` 到達まで pending に寄せる）
- 修正: タブ切替時の描画レース（ログが空になる / 2回クリックが必要になる）を修正（Webview 側でセッション別の blocks を保持）

## 0.1.13

- 追加: 入力画像をチャット履歴に「ギャラリー表示」（横2列）
  - 画像は `imageKey` でオフロードし、`SESSION_IMAGE_AUTOLOAD_RECENT=24` 枚のみ自動ロード
  - 表示時に縮小＆圧縮（最大辺 1024px / 目標 350KB）
  - Webview の Object URL を LRU でキャッシュ
- 改善: MCP image / Image view の表示を安定化（`file+.vscode-resource...` の 401 回避、`blob:` 描画 + CSP、オンデマンド読み込み/オフロード）
  - `globalStorage/images.v2` にキャッシュ（件数/容量上限で削除）
- 変更: Mentions は `@selection` のみ展開
  - 展開できない場合は送信を中断してエラー表示（サイレント送信しない）
  - その他の `@...` は解決せずプレーンテキストとして送信（コピペログ等でブロックしない）
- 追加: Status の rate limit 表示にホバーすると、リセット時刻を表示
- 変更: Interrupt を強化（turnId 未確定でも Stop/Interrupt を取りこぼしにくくする）
  - `turn/started` 到達後に割り込み送信
  - turnId 不明時は `thread/resume` で inProgress turn を探索して `turn/interrupt` を試行
  - backend kill/restart の Force Stop は廃止（`codexMine.interrupt.forceStopAfterMs` も削除）
- 修正: backend 停止/終了時にキャッシュ（thread/streamState 等）をクリーンアップし、`sending` / 承認待ち状態が残らないよう同期
- 追加: Agents（subagents）の一覧/候補取得（`.codex/agents` / `$CODEX_HOME/agents` をローカル走査）

## 0.1.12

- 追加: 拡張アイコン（`resources/icon.png`）

## 0.1.11

- 修正: 入力履歴の ↑/↓ ナビゲーションが全セッション共通になっていた（セッションごとに独立）
- 修正: `README.md:10` / `.env.local:23` のような「行番号付きファイル参照」がチャット内で開けないことがある（Markdownリンク / code トークン）
- 変更: 会話履歴（Runtime blocks）をワークスペースストレージにキャッシュしない（`thread/resume` で `~/.codex/sessions` から復元）
- 改善: Webview の full-state 更新（`refresh`）を間引き、ストリーミング中の更新連打で Extension Host が重くなるのを軽減

## 0.1.10

- 追加: チャット内の `@path/to/file` でも Ctrl/Cmd+Click でファイルを開ける
- 変更: `openFile` 失敗ダイアログ（`No matching result`）を廃止し、`vscode.open` に委譲
- 追加: Command カード（pre/meta）内のパスも Ctrl/Cmd+Click で開ける（出力が巨大な場合はリンク化を抑制）

## 0.1.9

- 変更: 承認 UI の Decline/Cancel で `turn/interrupt` を送って実行中を止める（次の入力に進める）
- 追加: チャット履歴内の `http://` / `https://` を Ctrl/Cmd+Click で開ける
- 改善: `@` のファイル検索を軽量化（2文字以上で検索、debounce 延長、キャンセル反応改善）

## 0.1.8

- 変更: Ctrl/Cmd+Hover 時のみファイルパスをリンク風表示（押しているだけでは表示しない）
- 修正: 実行中の停止（Esc）が入力欄フォーカス時に確実に効く

## 0.1.7

※このバージョンには、以前から実装されていたが CHANGELOG 未記載だった項目の追記も含む。

- 変更: legacy イベント（`codex/event/*`）の表示を最小許可リストに限定し、Command/Changes 等の重複表示を抑制
  - 許可: `token_count`, `mcp_startup_update`, `mcp_startup_complete`, `turn_aborted`, `list_custom_prompts_response`
- 修正: ブロックが de-dupe/削除された時に、Webview 側の残骸 DOM を掃除して重複表示を防止
- 追加: セッションをエディタタブで開ける（Session Panel / `Open Session (Editor Tab)`）
- 追加: セッションメニュー（タブ切替 / 非表示 / クローズ など）
- 追加: Runtime cache をワークスペース単位でクリア（`Clear Runtime Cache (Workspace)`）
- 追加: 承認（Approval）要求をチャット上にカード表示（Accept / Decline / Cancel / Accept (For Session)）
- 追加: Status で account / rate limits 等を表示
- 変更: 空のまま完了した `Reasoning` を表示しない（ノイズ削減）
- 追加: `Return to Bottom`（スクロールが Bottom にない時のみ表示。タブ切替時は自動で Bottom）
- 追加: 実行中でも画像の添付/ペーストができる（次の入力に備えて溜められる）
- 追加: チャット履歴内のファイルパスを Ctrl/Cmd+Click で開ける（見つからない場合は `No matching result`）
  - Ctrl/Cmd+Hover でリンク風表示
- 追加: Webview が隠れたり再生成されても、入力途中のテキストを保持（下書き保持）
- 変更: 不要な横スクロールが出ないよう調整（Webview 内の横方向スクロール抑制）

## 0.1.6

- 追加: 入力欄の `@` 補完に `@agents:{name}` を追加（codex-mine 実行時のみ）。ファイル候補より先に表示

## 0.1.5

- 修正: `thread/resume` でモデルを上書きしない（モデル不一致の警告を抑制）

## 0.1.4

- 修正: Resume の履歴復元が警告/デバッグ出力に邪魔されない
- 変更: Resume 一覧で時刻を先に表示
- 変更: デバッグ/Legacy イベント表示を折りたたみに変更（デフォルト閉じる）

## 0.1.3

- 追加: Resume 開始時に、New と同様に workspace folder を選べる（選択ディレクトリの履歴のみ表示）
- 変更: Resume 一覧から `modelProvider` / `cliVersion` を非表示

## 0.1.2

- 追加: ⚙ から CLI を `codex` / `codex-mine` / `auto` で切替（以降のデフォルト。適用には backend 再起動が必要）
  - `codex-mine` 選択時は `New` が常に codex-mine backend を使う（必要なら自動再起動）
- 追加: codex-mine 実行時のみ `/agents` を有効化（`.codex/agents` / `$CODEX_HOME/agents` から選び、`@name` を入力欄へ挿入）
- 追加: Resume（CHAT右上の Resume / `/resume`）
  - app-server の `thread/list` を使って履歴一覧から `thread/resume`
  - 履歴一覧は `CODEX_HOME` の全履歴を対象
- 変更: セッション名をリネームした場合、タブ/SESSIONS 表示から `#N` を外す（`#N` はデフォルト名の識別用途のみ）

## 0.1.1

- 追加: upstream 準拠で `skills/list` を呼び出し、`/skills` でスキルを挿入できる（repo-local `.codex/skills` も app-server 側で探索）

## 0.1.0

- 変更: Send ボタンをアイコン化。実行中は Stop（クリック / Esc）で `turn/interrupt`
- 変更: キャンセル（legacy `turn_aborted`）をノイズ扱いせず簡易表示（`Interrupted`）
- 変更: 入力欄を 1 行ベースにして自動伸長（上に伸びる）＋高さ調整
- 修正: 右上ステータス（チェック/スピナー）の位置ずれ
- 変更: Output を自動で開かない

## 0.0.7

- 追加: モデル一覧の取得と表示
- 追加: Reasoning effort 選択 UI

## 0.0.6

- 変更: MCP startup update イベントをグローバルステータスに表示

## 0.0.5

- 修正: shell-quote 依存が VSIX に含まれず起動時に失敗する

## 0.0.1

- 初期リリース（in-repo 開発版）
