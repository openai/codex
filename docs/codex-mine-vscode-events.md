# Codex-Mine (VS Code) イベント一覧と UI 取り扱い

このドキュメントは、`vscode-extension/` の Codex-Mine が受け取るイベント（JSON-RPC notification）と、Chat View 上での表示・状態反映のルールを整理したものです。

## 用語

- **v2 notification**: `thread/*`, `turn/*`, `item/*` のような method 名で届く通知。主に `vscode-extension/src/extension.ts` の `applyServerNotification()` で処理する。
- **legacy codex/event**: method が `codex/event/*` の通知で、payload 内 `msg.type` が実質のイベント種別。`applyCodexEvent()` / `applyGlobalCodexEvent()` で処理する。
- **global**: セッションに紐づかない通知。`applyGlobalNotification()` で処理し、Chat View の `globalBlocks`（画面上部の “Notice …”）に出す。

## v2 notifications（セッションスコープ）

Chat View の `blocks` / `statusText` / `latestDiff` に反映する。

- `thread/started`
  - UI: 何もしない（表示なし）
- `thread/compacted`
  - UI: `divider` ブロックを追加（例: `─ Worked for 21s ─` + `• Context compacted`）
- `turn/started`
  - UI: `sending=true`（入力欄を disable / スピナー相当）、表示はしない
- `turn/completed`
  - UI: `sending=false`、表示はしない（「Turn completed」はカードにしない方針）
- `thread/tokenUsage/updated`
  - UI: `statusText` 更新（`ctx remaining=xx% (remaining/ctx)`）
- `item/agentMessage/delta`
  - UI: `assistant` ブロックにストリーミング追記
- `item/reasoning/summaryTextDelta`
  - UI: `reasoning` ブロックの summary をストリーミング追記
- `item/reasoning/summaryPartAdded`
  - UI: `reasoning` ブロックの summary パート確保（表示は同じ）
- `item/reasoning/textDelta`
  - UI: `reasoning` ブロックの raw をストリーミング追記
- `item/commandExecution/outputDelta`
  - UI: `command` ブロック（details）内の Output をストリーミング追記
- `item/commandExecution/terminalInteraction`
  - UI: `command` ブロック（details）内の Stdin（terminal interaction）を追記
- `item/fileChange/outputDelta`
  - UI: `fileChange` ブロック（details）内の detail を追記
- `item/mcpToolCall/progress`
  - UI: `mcp` ブロック（details）内の progress を追記
- `turn/plan/updated`
  - UI: `plan` ブロック（details）を upsert（同一 turnId で更新）
- `turn/diff/updated`
  - UI: `latestDiff` を更新し、既存 `fileChange` ブロックの `hasDiff=true` にする
- `error`
  - UI: `error` ブロック（details）を追加
- `item/started` / `item/completed`
  - UI: `applyItemLifecycle()` によって item 種別ごとのブロックを upsert（reasoning/command/fileChange/mcp など）

## v2 notifications（グローバル）

Chat View の `globalBlocks` / `globalStatusText` に反映する。

- `windows/worldWritableWarning`
  - UI: `globalBlocks` に Notice として表示（details）
- `account/updated`
  - UI: `globalStatusText` 更新（例: `authMode=...`）
- `account/rateLimits/updated`
  - UI: `globalStatusText` 更新（primary/secondary/plan）
- `mcpServer/oauthLogin/completed`
  - UI: `success=false` のときのみ Notice として表示
- `account/login/completed`, `authStatusChange`, `loginChatGptComplete`, `sessionConfigured`
  - UI: 現状は “Other events (debug)” に寄せて表示（仕様未確定）
- その他
  - UI: `codex/event/*` なら legacy として処理し、それ以外は “Other events (debug)” に追記

## legacy codex/event（セッションスコープ）

method が `codex/event/*` のとき、payload の `msg.type` に応じて処理する。

- `exec_command_begin` / `exec_command_output_delta` / `terminal_interaction` / `exec_command_end`
  - UI: `command` ブロック（details）として表示（出力は折りたたみ）
- `token_count`
  - UI: `statusText` 更新（`ctx remaining=...`）
- `mcp_startup_complete`
  - UI: `failed/cancelled` が空でない場合のみ Notice として表示
- `task_started`, `task_complete`, `item_started`, `item_completed`, `user_message`, `agent_message*`
  - UI: v2 と重複するため無視（表示しない）
- その他
  - UI: “Other events (debug)” に追記（未対応イベントの露呈用）

## legacy codex/event（グローバル）

- `token_count`
  - UI: `globalStatusText` 更新
- `exec_command_*`, `terminal_interaction`
  - UI: 可能ならセッション側で出す方針のためグローバルでは無視
- `mcp_startup_complete`
  - UI: `failed/cancelled` が空でない場合のみ Notice として表示
- 「ノイズ扱い」イベント（reasoning の per-token delta など）
  - UI: 無視（表示しない）
- その他
  - UI: “Other events (debug)” に追記

## “Other events (debug)” の扱い

- 未対応イベントを **隠さずに露呈**させるための表示枠。
- 仕様が固まり次第、個別の UI（カード/ステータス/折りたたみ）へ昇格させ、ここへの出力を減らす。

## v1 / v2 とは？

この拡張には `vscode-extension/src/generated/v2/*` のように **プロトコル定義から生成された型**があり、ここでの `v2` は「現行の app-server 側の Thread/Turn/Item モデル（v2）」を指す。
一方で `codex/event/*` は過去互換の “legacy” で、同じ情報が別形式で重複して届くことがある。

## メンション（補足）

- `@selection`: 選択範囲を **ファイル相対パス + 行範囲**（例: `@src/app.ts#L10-L42`）として送信
- `@relative/path`: ファイルパスを送信（内容は展開しない）
- `@file:relative/path`: 互換（非推奨。`@relative/path` と同じ扱い）
