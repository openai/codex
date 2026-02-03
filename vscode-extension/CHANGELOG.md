# Changelog

この拡張はまだ初期段階です。互換性が壊れる変更が入る可能性があります。

## Unreleased

- **Sessions / Tabs**
  - セッションタブを backendId（opencode / codez）でフィルタリング。アクティブセッションと同じ backend のセッションのみ表示されるように変更
  - セッション番号付けを Sessions ツリーと一致（ワークスペース+backend 単位の連番）
- **OpenCode / Agent Mode**
  - OpenCodeのAgentモード（Build/Plan）切り替えをサポート
  - opencodeセッション時にモデルセレクターの左にAgentセレクターを表示
  - Agent選択時にAPIリクエストにagentパラメータを含めて送信
  - Agentセレクターのラベルをシンプル化（"default (CLI config)" → "default"）
- **OpenCode / Reasoning (Variant)**
  - OpenCodeモデルで推論強度（reasoning effort）の選択をサポート（none/minimal/low/medium/high/xhigh）
  - APIリクエストに `variant` パラメータを追加
  - モデル一覧に `supportedReasoningEfforts` を表示し、利用可能な推論強度を示す
  - 推論強度セレクターに "server default" を表示
- **OpenCode / Provider Filtering**
  - `enabled_providers` / `disabled_providers` 設定に対応し、プロバイダー一覧をフィルタリング
  - 接続済みプロバイダーのみを表示（サーバーが connected 情報を返す場合）
- **OpenCode / Reasoning UI Improvements**
  - 複数の reasoning パートを1つのUIブロックに統合（opencode web と同じ表示方式）
  - reasoning の time.end を反映し、完了時にスピナーが残り続ける問題を修正
- **OpenCode / Tool & Message Handling**
  - step より前に到着する tool パートを適切に処理
  - SSE イベントパースを改善（CRLF/LF の両方に対応）
  - 空の assistant メッセージを送信しないように修正
  - /message 応答と SSE の二重反映でカードが重複・順序が崩れる問題を修正（SSE を唯一の描画ソースに統一）
  - OpenCode サーバへの接続エラー時に opencode backend を破棄してキャッシュをクリーンアップ（次の操作で再起動できる状態に）
  - delta が空の unknown part で空カード（黒いカード）が出る問題を修正
  - カードの並び替えが UI 上で反映されない問題を修正（OpenCodeブロックのみ順序に追従して再配置）
  - OpenCode の Part（file/patch/agent/snapshot/retry/compaction/subtask）をカード表示し、複数パートでIDが衝突して潰れる問題を修正
  - Notice（起動時カード）が更新のたびに末尾へ移動してしまう問題を修正（Noticeは先頭に固定）
- **OpenCode / Permissions**
  - permission.asked を UI に表示し、Allow once / Always allow / Reject の応答をサポート（これが無いとツールが待ち状態で止まり得る）
- **OpenCode / Server Process**
  - opencode サーバープロセスを拡張内で共有（ワークスペースごとに `opencode serve` が増殖してフリーズ/競合しやすい問題を緩和）
- **OpenCode / Server Info**
  - 起動時のサーバー情報カードから Config keys と Skills の一覧表示を削除（エラー時のみ表示）
  - codez/codex セッションで OpenCode の起動時カードが混ざる問題を修正（OpenCode started としてbackend/cwdでフィルタ）
- **Skills**
  - `No skills found` の案内に `./.codex/config.toml`（project）も明記（codez の config layering を反映）

## 0.2.7

- **OpenCode**
  - opencode 起動時にサーバ情報（version/dir/providers/config keys/skills）をカード表示
- **Sessions / Notices**
  - `Thread started` の de-dupe を backend-aware にし、opencode と codez/codex の表示が混ざらないように修正

## 0.2.6

- **Models / Defaults**
  - `default` のラベルに、CLI の既定（`CODEX_HOME/config.toml` の provider/model）を表示（codex/codez）
  - opencode セッションの `default` が `opencode config` 由来か、OpenCode の provider 既定由来かを区別して表示
- **OpenCode**
  - `/provider` の `default` を反映して、モデル一覧で provider 既定を `isDefault` としてマーク
  - `/config.model` が未設定（自動選択）の場合でも、`connected` provider と `default` から既定モデルを推定して表示
- **Sessions / Reload**
  - Reload 後など履歴未ロード時に、タブクリックで自動 Resume せず「Load history」で明示的に復元できるように
- **Performance / Debug**
  - Webview のセッション別ブロックキャッシュを LRU で上限化（メモリ肥大を抑制）
  - Output の debug ログ（Unhandled events）を上限でトリム（無制限に増えない）

## 0.2.5

- **Docs**
  - 拡張の説明（README）を刷新し、`codex` / `codez` / `opencode` の対応範囲と制約を明確化

## 0.2.4

- **OpenCode / WSL 安定性**
  - `server.heartbeat` / `session.updated` / `session.status` を Unhandled として出さず黙って無視（Output のノイズ削減）
  - `session.status`（busy/idle）を追跡し、busy 中は Rewind を抑止して「Stop してから」表示
  - `fetch failed` 時に `err.cause` まで Output に出力し、WSL 環境での根因特定を容易化

## 0.2.3

- **OpenCode 表示改善**
  - `step` と `tool` を「Step」カードに集約し、ToolUse のスパム表示（謎のMCP）を抑制
  - `Step (stop)` のような最終ステップはカード化せず、assistant 末尾のメタ情報（cost/tokens）に吸収
  - 一部ツール（例: `glob`, `read`）の input をツール見出しにプレビュー表示（カードを開かなくても分かる）

## 0.2.2

- **Sessions / Backends**
  - セッション作成時に backend（codex/codez/opencode）を選択できるようにし、同一 workspace で複数 backend を常駐可能に
  - 旧セッション形式（v1）を新形式（v2）へ移行するコマンド `codez.migrateSessionsV1`
- **OpenCode backend**
  - opencode server（`opencode serve`）をバックエンドとして起動・接続できる設定（`codez.opencode.*`）
  - opencode backend のモデル一覧が欠けることがある問題を修正（provider の models 形式差に対応）
  - opencode backend のメッセージ parts を履歴に反映（reasoning/tool 相当は raw JSON を表示）
- **Settings / Accounts**
  - Settings をアクティブセッションの backendId ベースで分岐し、opencode セッションで Accounts 等が破綻することがある問題を修正
  - Settings から `Workspace defaults` を削除（backend は New/Start Backend で明示選択）
  - Settings から同一スレッドを codex↔codez で開き直せる導線（opencode への引き継ぎは不可）
  - Settings の見た目/操作性を改善（行間、hover/focus、ボタン配色、Backend 操作行のレイアウト）
  - `account/list` / `account/switch`（複数アカウント切替）は codez セッションのみ対応のため、codex/opencode セッションでは呼ばず案内表示する
  - codez 限定機能を codex/opencode セッションで実行しようとした場合、エラーではなく「codez セッションのみ対応」旨を案内表示する
- **Approvals / Compact**
  - 承認（approval）カードに「実行予定コマンド（`$ ...`）」と `cwd` を表示（item が未到着でも request params から表示）
  - `/compact`（`thread/compact`）で手動 compaction を起動できるように復活
- **MCP**
  - `/mcp` でセッションごとの MCP server 一覧をカード表示（起動時と同様の状態アイコン）

## 0.2.1

- **Misc**
  - Buy Me a Coffee のリンクを `package.json` / README に追加

## 0.2.0

- **破壊的変更**
  - 設定キー・コマンド ID・View ID を `codexMine.*` から `codez.*` に変更（旧キーは非互換）
- **Accounts / Settings**
  - 複数アカウントの切替（TUI / app-server / VSCode を横断して対応）
  - Settings のアカウント管理（`/account`）、および CLI variant の切替 UI
  - app-server 経由のログインフロー（Settings から起動）
  - アクティブアカウント未設定時は legacy auth をデフォルトにする
- **Turns / Approvals**
  - rewind を `thread/rollback` ベースに切り替え
  - セッション承認（approval）UI の構造/更新ロジックを改善
- **UI**
  - mine-only の UI を selected `cli.variant` に応じて出し分け
  - モデル/推論強度の選択がセッションごとに独立せず、他セッションに波及することがある問題を修正
  - チャット内の長いパス/URL が折り返されず横にはみ出すことがある問題を修正（自動リンク化されたファイル/URL 表示を含む）
  - 長いリンクの折り返しを改善
  - セッションタブのプロジェクト（workspace）グループ枠の横幅をタブ幅に合わせ、見出し（プロジェクト名）は幅内で省略表示するように修正

## 0.1.16

- **Sessions / Tabs**
  - SESSIONS/Chat のセッションをプロジェクト（workspace folder）単位で色分け
    - Chat のセッションタブをプロジェクトごとに角丸グループ枠で囲って表示（Chrome のタブグループ風）
    - SESSIONS ツリーにも同色のマーカーを表示
  - プロジェクト色の手動設定（workspaceFolderUri 単位で永続化）
    - Chat のグループ見出しを右クリック/二本指タップして色を選択（QuickPick）
    - 「自動」を選ぶと手動設定を解除
  - Chat 上部 UI の視認性調整（ボタン/ラベルのサイズ、ラベルの省略表示）
- **MCP**
  - セッション開始時の `MCP servers` 表示が他プロジェクトと混ざることがある問題を修正
    - セッションの `cwd` に対して `mcpServerStatus/list` を問い合わせ、設定上有効な MCP のみを表示
- **AskUserQuestion**
  - AskUserQuestion（`ask_user_question` tool）の UI をサポート（デフォルト無効。`config.toml` で明示的に有効化した場合のみモデルへ露出）
    - 推奨: `[features].ask_user_question_tool = true`（互換: `[tools].ask_user_question = true`）
    - 質問カードの表示と回答の送信
    - 選択肢（options）のクリック選択
    - 回答（answers）をチャット履歴に保持
- **Webview / Performance**
  - Webview が非表示の間は state 更新を送らず、ACK 待ちもしない（表示に戻ったら `refresh` で追いつく）
  - state 更新 ACK のタイムアウトは UI エラー扱いせず、デバッグログに出す
- **Links**
  - チャット内のファイル参照リンク（Ctrl/Cmd+Click で開く）を安定化
    - `+` を含むパスでもリンクが途中で切れない
    - `README.md` などが外部リンク扱いになってブラウザが開く誤動作を防止
    - コマンド/ツール出力では、作業ディレクトリ（`cwd`）を使って `/` なしファイル名も見つけやすくする（候補が多すぎる場合は明示エラー）
- **Streaming**
  - assistant のストリーミング完了後に `<pre>` 表示のまま固定され、最終メッセージが Markdown として再描画されないことがある問題を修正

## 0.1.15

- **Links**
  - チャット内のファイルパスリンク化で日本語（Unicode）をサポート
    - 通常テキスト: 全角スペース（`　`）と `・` を含むパスをリンク化（半角スペースは区切りとして扱う）
    - コードブロック: 半角スペースを含むパスもリンク化
- **Backend compatibility**
  - codex backend 利用時は、未対応の機能（Rewind/Edit, /compact, Reload）を UI から無効化
- **UI**
  - 上部のボタンを整理（`Status` / `Open Latest Diff` を削除し、`/status` / `/diff` から実行）

## 0.1.14

- **Sessions / Turns**
  - セッションの `reload` / `rewind` / `undo` をサポート
  - 過去 turn の Edit（rewind）は会話のみを巻き戻す（ファイル/作業ツリーは巻き戻らない。Issue #23）
- **Compact**
  - `/compact`（サーバー側の `thread/compact`）の実行状況を UI で見える化（Context カードを即時表示、進行中はスピナー、完了はチェック。失敗は×＋エラー）
- **Webview / Performance**
  - 大量の `delta` 受信時に UI 更新が詰まって止まるケースを軽減（更新の coalesce、差分追記の効率化）
  - タブ切替時の描画レース（ログが空になる / 2回クリックが必要になる）を修正（Webview 側でセッション別の blocks を保持）
- **Resume**
  - `thread/resume` 時に `cwd` / model 等を上書きしない（進行中 turn のストリームを壊しにくくする。loaded conversation は fast-path で復元）
  - セッション切替（`selectSession`）で裏の `thread/resume` を実行しない（進行中 turn のストリーム競合を回避）
- **Mentions**
  - `@` のファイル検索候補が更新されずに詰まることがある問題を修正
- **Interrupt**
  - Interrupt の挙動を見直し（turnId 不明時に `thread/resume` で inProgress turn を探索しない。`turn/started` 到達まで pending に寄せる）

## 0.1.13

- **Images**
  - 入力画像をチャット履歴に「ギャラリー表示」（横2列）
    - 画像は `imageKey` でオフロードし、`SESSION_IMAGE_AUTOLOAD_RECENT=24` 枚のみ自動ロード
    - 表示時に縮小＆圧縮（最大辺 1024px / 目標 350KB）
    - Webview の Object URL を LRU でキャッシュ
  - MCP image / Image view の表示を安定化（`file+.vscode-resource...` の 401 回避、`blob:` 描画 + CSP、オンデマンド読み込み/オフロード）
    - `globalStorage/images.v2` にキャッシュ（件数/容量上限で削除）
- **Mentions**
  - Mentions は `@selection` のみ展開
    - 展開できない場合は送信を中断してエラー表示（サイレント送信しない）
    - その他の `@...` は解決せずプレーンテキストとして送信（コピペログ等でブロックしない）
- **Status**
  - Status の rate limit 表示にホバーすると、リセット時刻を表示
- **Interrupt**
  - Interrupt を強化（turnId 未確定でも Stop/Interrupt を取りこぼしにくくする）
    - `turn/started` 到達後に割り込み送信
    - turnId 不明時は `thread/resume` で inProgress turn を探索して `turn/interrupt` を試行
    - backend kill/restart の Force Stop は廃止（`codez.interrupt.forceStopAfterMs` も削除）
- **Backend lifecycle**
  - backend 停止/終了時にキャッシュ（thread/streamState 等）をクリーンアップし、`sending` / 承認待ち状態が残らないよう同期
- **Agents**
  - Agents（subagents）の一覧/候補取得（`.codex/agents` / `$CODEX_HOME/agents` をローカル走査）

## 0.1.12

- **Misc**
  - 拡張アイコン（`resources/icon.png`）

## 0.1.11

- **Composer**
  - 入力履歴の ↑/↓ ナビゲーションが全セッション共通になっていた問題を修正（セッションごとに独立）
- **Links**
  - `README.md:10` / `.env.local:23` のような「行番号付きファイル参照」がチャット内で開けないことがある問題を修正（Markdown リンク / code トークン）
- **Storage / Resume**
  - 会話履歴（Runtime blocks）をワークスペースストレージにキャッシュしない（`thread/resume` で `~/.codex/sessions` から復元）
- **Webview / Performance**
  - Webview の full-state 更新（`refresh`）を間引き、ストリーミング中の更新連打で Extension Host が重くなるのを軽減

## 0.1.10

- **Links**
  - チャット内の `@path/to/file` でも Ctrl/Cmd+Click でファイルを開ける
  - `openFile` 失敗ダイアログ（`No matching result`）を廃止し、`vscode.open` に委譲
  - Command カード（pre/meta）内のパスも Ctrl/Cmd+Click で開ける（出力が巨大な場合はリンク化を抑制）

## 0.1.9

- **Approvals / Interrupt**
  - 承認 UI の Decline/Cancel で `turn/interrupt` を送って実行中を止める（次の入力に進める）
- **Links**
  - チャット履歴内の `http://` / `https://` を Ctrl/Cmd+Click で開ける
- **Mentions**
  - `@` のファイル検索を軽量化（2文字以上で検索、debounce 延長、キャンセル反応改善）

## 0.1.8

- **Links**
  - Ctrl/Cmd+Hover 時のみファイルパスをリンク風表示（押しているだけでは表示しない）
- **Interrupt**
  - 実行中の停止（Esc）が入力欄フォーカス時に確実に効く

## 0.1.7

※このバージョンには、以前から実装されていたが CHANGELOG 未記載だった項目の追記も含む。

- **Events / Noise control**
  - legacy イベント（`codex/event/*`）の表示を最小許可リストに限定し、Command/Changes 等の重複表示を抑制
    - 許可: `token_count`, `mcp_startup_update`, `mcp_startup_complete`, `turn_aborted`, `list_custom_prompts_response`
  - 空のまま完了した `Reasoning` を表示しない（ノイズ削減）
- **Webview / Performance**
  - ブロックが de-dupe/削除された時に、Webview 側の残骸 DOM を掃除して重複表示を防止
  - 不要な横スクロールが出ないよう調整（Webview 内の横方向スクロール抑制）
- **Sessions**
  - セッションをエディタタブで開ける（Session Panel / `Open Session (Editor Tab)`）
  - セッションメニュー（タブ切替 / 非表示 / クローズ など）
  - Runtime cache をワークスペース単位でクリア（`Clear Runtime Cache (Workspace)`）
- **Approvals**
  - 承認（Approval）要求をチャット上にカード表示（Accept / Decline / Cancel / Accept (For Session)）
- **Status**
  - Status で account / rate limits 等を表示
- **UX**
  - `Return to Bottom`（スクロールが Bottom にない時のみ表示。タブ切替時は自動で Bottom）
  - 実行中でも画像の添付/ペーストができる（次の入力に備えて溜められる）
  - Webview が隠れたり再生成されても、入力途中のテキストを保持（下書き保持）
- **Links**
  - チャット履歴内のファイルパスを Ctrl/Cmd+Click で開ける（見つからない場合は `No matching result`）
    - Ctrl/Cmd+Hover でリンク風表示

## 0.1.6

- **Mentions**
  - 入力欄の `@` 補完に `@agents:{name}` を追加（codez 実行時のみ）。ファイル候補より先に表示

## 0.1.5

- **Resume**
  - `thread/resume` でモデルを上書きしない（モデル不一致の警告を抑制）

## 0.1.4

- **Resume**
  - Resume の履歴復元が警告/デバッグ出力に邪魔されない
  - Resume 一覧で時刻を先に表示
- **Events / Debug**
  - デバッグ/Legacy イベント表示を折りたたみに変更（デフォルト閉じる）

## 0.1.3

- **Resume**
  - Resume 開始時に、New と同様に workspace folder を選べる（選択ディレクトリの履歴のみ表示）
  - Resume 一覧から `modelProvider` / `cliVersion` を非表示

## 0.1.2

- **Settings / CLI**
  - ⚙ から CLI を `codex` / `codez` / `auto` で切替（以降のデフォルト。適用には backend 再起動が必要）
    - `codez` 選択時は `New` が常に codez backend を使う（必要なら自動再起動）
- **Agents**
  - codez 実行時のみ `/agents` を有効化（`.codex/agents` / `$CODEX_HOME/agents` から選び、`@name` を入力欄へ挿入）
- **Resume**
  - Resume（CHAT 右上の Resume / `/resume`）
    - app-server の `thread/list` を使って履歴一覧から `thread/resume`
    - 履歴一覧は `CODEX_HOME` の全履歴を対象
- **Sessions**
  - セッション名をリネームした場合、タブ/SESSIONS 表示から `#N` を外す（`#N` はデフォルト名の識別用途のみ）

## 0.1.1

- **Skills**
  - upstream 準拠で `skills/list` を呼び出し、`/skills` でスキルを挿入できる（repo-local `.codex/skills` も app-server 側で探索）

## 0.1.0

- **Composer / Interrupt**
  - Send ボタンをアイコン化。実行中は Stop（クリック / Esc）で `turn/interrupt`
  - 入力欄を 1 行ベースにして自動伸長（上に伸びる）＋高さ調整
- **Events**
  - キャンセル（legacy `turn_aborted`）をノイズ扱いせず簡易表示（`Interrupted`）
- **UI**
  - 右上ステータス（チェック/スピナー）の位置ずれを修正
  - Output を自動で開かない

## 0.0.7

- **Models**
  - モデル一覧の取得と表示
  - Reasoning effort 選択 UI

## 0.0.6

- **MCP**
  - MCP startup update イベントをグローバルステータスに表示

## 0.0.5

- **Fixes**
  - shell-quote 依存が VSIX に含まれず起動時に失敗する問題を修正

## 0.0.1

- 初期リリース（in-repo 開発版）
