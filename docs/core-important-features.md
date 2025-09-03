# codex-rs/core/src 重要機能まとめ

このドキュメントは `codex-rs/core/src` の実装における「重要な機能」と、その相互作用の要点を俯瞰できるよう整理したものです。運用・拡張・バグ調査時の入口として活用できます。

## 概要
- 目的: モデル対話（ストリーミング）、ツール呼び出し（shell/apply_patch/MCP 等）、安全な実行（サンドボックス/承認）、会話管理、構成管理を提供する中核ライブラリ。
- 中核構造: `Codex` セッション（イベント駆動）＋ モデルクライアント（Responses/Chat）＋ ツール定義と安全実行層（seatbelt/landlock）＋ MCP 連携。
- 主要責務:
  - セッション生成・送受信・承認やストリーミングの制御
  - 実行ポリシー（SandboxPolicy）と承認方針（AskForApproval）に基づく安全なコマンド/パッチ適用
  - MCP ツール群の集約と呼び出し、OpenAI tools へのマッピング

## 中核コンポーネント
- セッション管理（`codex.rs`）
  - `Codex::spawn/submit/next_event` でセッション生成・送信・受信。
  - `Session` がターン文脈（モデルクライアント、CWD、承認/サンドボックス方針、利用可能ツール）を保持。
  - `Session::request_command_approval` など承認リクエストイベントの発行。
- モデルクライアント（`client.rs` / `client_common.rs`）
  - OpenAI Responses/Chat API へのストリーミング実行、イベント（`ResponseEvent`）へ正規化。
  - `Prompt` が入力整形（BASE_INSTRUCTIONS/ユーザー指示/ツール列）を担当。
  - 再試行/バックオフ、トークン使用などの付帯情報を集約。
- Chat Completions 互換実装（`chat_completions.rs`）
  - Chat SSE を Responses 風に集約して、共通のイベントストリームへ橋渡し。

## ツール統合・提示（OpenAI Tools）
- `openai_tools.rs`
  - `shell`/`apply_patch`/`update_plan`/`web_search`/`view_image` と MCP ツールを、モデルファミリ/承認・サンドボックス方針に応じて構成。
  - JSON Schema 正規化（欠落 `type` の補完など）により、異種ツール定義を API 互換へ統一。
  - ストリーム可能シェル（`exec_command`）を機能として公開可能。
- `plan_tool.rs`
  - 進行中のプラン更新ツール（TODO 的）を Function として提供、イベントに橋渡し。

## apply_patch と安全性
- `apply_patch.rs`
  - `assess_patch_safety`（後述）に基づき、自動承認/要承認/拒否を分岐。
  - 承認/自動承認時は `exec` 経由（`CODEX_APPLY_PATCH_ARG1`）で実行委譲、または Function 出力で応答。
- `tool_apply_patch.rs`
  - Freeform（grammar）/Function（JSON）両形式のツール定義を提供（OSS モデル向け Function 対応）。

## 実行とサンドボックス
- 実行中核（`exec.rs`）
  - `SandboxType`（None/Seatbelt/LinuxSeccomp）ごとに適切に起動し、ストリーミング/集約出力、タイムアウト、シグナル、サンドボックス起因エラーを扱う。
  - 出力は `ExecCommandOutputDeltaEvent` で段階送信、総数上限や集約の扱いあり。
- macOS Seatbelt（`seatbelt.rs`）
  - `/usr/bin/sandbox-exec` によるポリシー生成（書込可能ルート、読み取り、ネット許可）と安全実行。
- Linux Landlock + seccomp（`landlock.rs`）
  - 補助バイナリ `codex-linux-sandbox` を JSON 方針で起動し、同等の権限制御を実現。
- プロセス起動共通（`spawn.rs`）
  - 環境変数のクリーン設定、ネットワーク無効化 ENV、stdin/stdout のポリシー、Linux の親死検知（PR_SET_PDEATHSIG）など。

## コマンド安全判定
- `safety.rs`
  - 既知安全/ユーザー承認/サンドボックス可用性/承認方針に基づいて `AutoApprove/AskUser/Reject` を決定。
  - apply_patch の書込先がワークスペース許可内か確認（move/rename を含む）。
- `is_safe_command.rs`
  - `ls`/`git status` 等の既知安全コマンドや `bash -lc` の安全サブセット（単語/引用/制限演算子）を許容。
  - `find/rg` の危険オプションを検出し自動承認しない。

## インタラクティブ実行（ストリーム可能シェル）
- `exec_command/*`
  - PTY ベースでセッションを作成・入力（stdin）・出力（broadcast）・終了状態を管理。
  - `SessionManager` により複数セッションを同時管理し、Function ツールとして公開。

## MCP（Model Context Protocol）連携
- `mcp_connection_manager.rs`
  - 複数 MCP サーバの起動/接続、ツール一覧の集約、FQ 名（`server__tool`）/長さ制限/重複回避。
- `mcp_tool_call.rs`
  - MCP ツールの Begin/End をイベント通知し、結果を `ResponseItem` に橋渡し。

## 会話・履歴
- `conversation_manager.rs`
  - 会話生成/参照/フォーク/終了。最初の `SessionConfigured` を受信して整合性確認。
- `conversation_history.rs` / `message_history.rs`
  - `~/.codex/history.jsonl` への追記・ロック・権限確認、メタデータ取得やオフセット読み出し。

## 環境文脈・通知
- `environment_context.rs`
  - `<environment_context>` XML（cwd/承認/サンドボックス/ネット/シェル）を生成しモデルへ提示。
- `user_notification.rs`
  - ターン完了時などに外部通知コマンドを起動（JSON 引数付）。

## 設定・プロバイダ
- `config.rs` / `config_types.rs`
  - `~/.codex/config.toml` と CLI オーバーライド/強型オーバーライドのマージ。
  - `SandboxPolicy`（ReadOnly/WorkspaceWrite/DangerFullAccess）、`ShellEnvironmentPolicy`、MCP 設定、履歴ポリシー、TUI 設定等。
- `model_provider_info.rs` / `openai_model_info.rs` / `model_family.rs`
  - プロバイダごとのヘッダ/URL/認証/リトライ・ストリーム再接続閾値。
  - モデルファミリ特性（reasoning summaries 可否、local_shell 利用、apply_patch の形式）と既定値。

## Git 連携
- `git_info.rs`
  - リポジトリ判定、SHA/ブランチ/URL 取得（タイムアウト付き）、リモートに近い SHA との diff 取得、信頼パス解決（worktree 対応）。

## コマンド可視化
- `parse_command.rs`
  - モデル生成コマンドを「検索/一覧/フォーマット/テスト/リンタ/不明」へ要約（`bash -lc` 解析を活用）。

## 端末/シェル統合
- `terminal.rs` / `shell.rs` / `bash.rs`
  - Zsh/PowerShell の既定起動引数整形、`bash -lc` スクリプト抽出、TUI 側が利用する端末抽象。

---

## 重要な相互作用（設計上のキモ）
- 承認 × サンドボックス
  - `safety.rs` の判定により、`exec.rs`（Seatbelt/Landlock）で実行、承認が必要なケースを UI/イベント越しに明示。
- ツール提示 × モデル特性
  - `openai_tools.rs` ＋ `model_family.rs` でモデル向け最適構成（`apply_patch` 形式、`local_shell`、`view_image` 等）を提示。`Prompt` がインストラクション補助。
- ストリーミング × リトライ制御
  - `client.rs`/`chat_completions.rs` で SSE 切断や高負荷時のリトライ、アイドルタイムアウト、指数バックオフを実装。
- インタラクティブ実行
  - `exec_command/*` により REPL/長時間プロセスの入力/出力/終了制御を安全にセッション管理。

---

## 関連ファイル（カテゴリ別）
- エントリ/会話: `codex.rs`, `conversation_manager.rs`, `conversation_history.rs`, `message_history.rs`
- 実行/安全: `exec.rs`, `spawn.rs`, `seatbelt.rs`, `landlock.rs`, `safety.rs`, `is_safe_command.rs`
- ツール/適用: `openai_tools.rs`, `apply_patch.rs`, `tool_apply_patch.rs`, `plan_tool.rs`, `exec_command/*`, `mcp_connection_manager.rs`, `mcp_tool_call.rs`
- モデル/設定: `client.rs`, `client_common.rs`, `chat_completions.rs`, `model_provider_info.rs`, `openai_model_info.rs`, `model_family.rs`, `config.rs`, `config_types.rs`
- 周辺: `environment_context.rs`, `git_info.rs`, `shell.rs`, `bash.rs`, `terminal.rs`, `util.rs`, `error.rs`, `flags.rs`, `project_doc.rs`, `rollout.rs`, `user_agent.rs`

---

## 補足
- テストは多数のモジュールに併設されており、挙動の詳細は該当ファイルの `#[cfg(test)]` を参照すると早いです。
- Linux/macOS でサンドボックス実装が分岐するため、OS 依存の挙動差は `seatbelt.rs`/`landlock.rs`/`spawn.rs` 近辺を確認してください。
- MCP サーバの起動/失敗は `mcp_connection_manager.rs` が集約（エラーは会話開始後のイベントで通知）。

---

## 詳細編

### アーキテクチャとライフサイクル
- セッション生成: `Codex::spawn` が `ConfigureSession` を構築し、`McpConnectionManager::new` で MCP を起動。`ModelClient` と `TurnContext` を初期化。
- 初期イベント: `SessionConfigured` を必ず最初に送出。履歴（オプション）・環境文脈を記録。
- 送信/受信: `submit()` で `Submission` を送信、`next_event()` で `Event` を順次取得。
- モデルストリーム: `ModelClient::stream()` が Responses/Chat へ振分。`show_raw_agent_reasoning` で出力モード調整。

### イベント流とメッセージ
- 出力系: `OutputTextDelta`（本文差分）、`ReasoningSummaryDelta/PartAdded`、`Completed{response_id, token_usage}`。
- エラー系: `Error`, `StreamError`（SSE切断など）。
- ツール系: `ExecApprovalRequest`, `PatchApplyBegin/End`, `McpToolCallBegin/End`。
- 履歴系: `TaskStarted`, `TaskComplete`, `TurnDiff`（差分情報）。

### ツール構成の決定ロジック
- Shell:
  - Streamable: `use_experimental_streamable_shell_tool=true` → `exec_command` を2ツール（起動+stdin）で公開。
  - Local shell: `model_family.uses_local_shell_tool=true` → `local_shell`（説明不要）
  - Function shell: それ以外は `shell`（Function）。`OnRequest` 時は説明文にエスカレーション手順を含む variant を使用。
- apply_patch:
  - `model_family.apply_patch_tool_type`（指定が最優先）。未指定で `include_apply_patch_tool=true` は Freeform（grammar）。
- 追加: `web_search_request`/`include_view_image_tool` が true のとき各ツールを追加。
- MCP: 取得したツールは FQ 名（`server__tool`）で安定化・重複排除。

### apply_patch 実行フロー
1. ツール呼出（Freeform/Function）。
2. `assess_patch_safety` で `AutoApprove/AskUser/Reject` 判定。
3. 自動/明示承認時は `exec` に委譲（`--codex-run-as-apply-patch`）。
4. `convert_apply_patch_to_protocol` で UI 向け `FileChange` に変換。

### 実行詳細（主な定数・挙動）
- 既定タイムアウト: `DEFAULT_TIMEOUT_MS=10_000`。
- 逐次送信上限: `MAX_EXEC_OUTPUT_DELTAS_PER_CALL=10_000`。
- サンドボックス起因推定: `is_likely_sandbox_denied`（127等の除外）。
- 環境: `env_clear` 後に必要最小限を注入。ネット無効時は `CODEX_SANDBOX_NETWORK_DISABLED=1`。
- Linux: `PR_SET_PDEATHSIG=SIGTERM`、`kill_on_drop(true)`。

### Seatbelt ポリシー要点
- 基本ポリシー（同梱 `seatbelt_base_policy.sbpl`）+ 読み取り/書き込み/ネットの可否を合成。
- `WorkspaceWrite`: 書込ルートごとに `-DWRITABLE_ROOT_i` を付与し、`.git` 等は `require-not` で除外。

### Linux Landlock 補助バイナリ
- 引数: `[cwd, json(sandbox_policy), "--", <command...>]`。
- `arg0` を `codex-linux-sandbox` に固定。

### 判定分岐（要点）
- `UnlessTrusted` → 常に `AskUser`（未信頼）。
- `DangerFullAccess` → `AutoApprove{None}`。
- `OnRequest`×`ReadOnly|WorkspaceWrite`: `with_escalated_permissions=true` は `AskUser`。それ以外はサンドボックス可用なら `AutoApprove`、不可なら `AskUser`。
- `Never|OnFailure`×`ReadOnly|WorkspaceWrite`: サンドボックス可用なら `AutoApprove`、不可で `OnFailure` は `AskUser`、`Never` は `Reject`。

### 既知安全コマンド（抜粋）
- 基本: `cat` `cd` `echo` `false` `grep` `head` `ls` `nl` `pwd` `tail` `true` `wc` `which`
- `find`: `-exec/-execdir/-ok/-okdir/-delete/-fls/-fprint/-fprint0/-fprintf` を含むと非安全。
- `rg`: `--pre` `--hostname-bin` `--search-zip`/`-z` を含むと非安全。
- 特例: `sed -n {N|M,N}p FILE` は安全。

### インタラクティブ実行（exec_command）
- パラメータ:
  - `ExecCommandParams{ cmd, yield_time_ms=10_000, max_output_tokens=10_000, shell="/bin/bash", login=true }`
  - `WriteStdinParams{ session_id, chars, yield_time_ms=250, max_output_tokens=10_000 }`
- 返り値:
  - `ExecCommandOutput{ wall_time, exit_status(Exited/Ongoing), original_token_count, output }`（長大時は中央省略）。
- 実装:
  - PTY 読み取りを broadcast（購読遅延は許容、遅延時はスキップ）。終了検知後は短いグレース期間で残出力を吸収。

### MCP 詳細
- 起動: サーバごとに並列起動。無効名は事前検証しスキップ。
- ツール名: 64文字超は SHA-1 で接尾辞を付与、衝突は warn で除外。
- 呼出: `call_tool(server, tool, arguments(JSON), timeout)`。`handle_mcp_tool_call` は Begin/End をイベントで周知し、所要時間を記録。

### 履歴実装の要点
- パス: `~/.codex/history.jsonl`。ディレクトリは起動時に作成。
- 書込: `0600`（UNIX）。ファイルロックはリトライ付で取得し 1 行単位で追記。
- メタ: inode（Unix）と行数（改行カウント）を非同期で収集。

### EnvironmentContext 仕様
- `<environment_context> ... </environment_context>` に cwd/承認/サンドボックス/ネット/シェルを XML で埋め込み、会話先頭に投入可。

### Config の主なフィールド
- モデル: `model`, `model_family`, `model_context_window`, `model_max_output_tokens`。
- 方針/ツール: `approval_policy`, `sandbox_policy`, `shell_environment_policy`, `include_plan_tool`, `include_apply_patch_tool`, `tools_web_search_request`, `use_experimental_streamable_shell_tool`, `include_view_image_tool`。
- 表示/履歴: `hide_agent_reasoning`, `show_raw_agent_reasoning`, `history`, `disable_response_storage`, `disable_paste_burst`。
- その他: `notify`, `cwd`, `project_doc_max_bytes=32KiB`, `codex_home`, `chatgpt_base_url`, `experimental_resume`, `responses_originator_header`, `preferred_auth_method`, `model_verbosity`。

### Provider の再試行/ストリーム設定
- 既定: `request_max_retries=4`, `stream_max_retries=5`, `stream_idle_timeout_ms=300_000`。
- 上限: `MAX_* = 100`。個別プロバイダで上書き可。

### モデルファミリの振る舞い
- `gpt-5`: reasoning summaries 対応、`text.verbosity` を含める。
- `gpt-4.1`: apply_patch 追加指示が有効。
- `codex-mini-latest`: `local_shell` ツール、reasoning summaries 対応。
- `gpt-oss`: apply_patch は Function ツール形式。

### Git 実装の要点
- タイムアウト防止: ほぼ全 git コマンドを `timeout(5s)` で保護。
- 既定ブランチ特定: `refs/remotes/<remote>/HEAD` → `git remote show` → ローカル候補（main/master）。
- diff 生成: 追跡ファイルの diff + 未追跡は `/dev/null` と比較して追補（`--no-index --binary`）。

### 解析戦略（parse_command）
- `bash -lc` 内を tree-sitter でトークン化し、語/数/クォートのみのコマンド列を許可。
- 安全演算子（`&&`, `||`, `;`, `|`）のみ許容。重複はデデュープ。

### Shell 補助
- zsh: zshrc が存在すれば `source <zshrc> && (<script>)` で環境を整えて実行。
- PowerShell: bash スクリプト検出時は `bash_exe_fallback` で代替、それ以外は `-NoProfile -Command` で実行。

### 拡張のヒント
- 新規 MCP サーバ: `config.toml` の `mcp_servers` に追記 → `McpConnectionManager` が自動発見・集約。
- 新規ツール: `openai_tools.rs` に追加し、`ToolsConfig` 分岐へ組込み。必要なら `JsonSchema` を拡張・正規化。
- 新規プロバイダ: `model_provider_info.rs` の既定に追加 or `config.toml` の `model_providers` に記述（`env_key`/ヘッダ等）。
- 安全コマンド見直し: `is_safe_command.rs` のリスト/判定を更新（テスト追記推奨）。

### 既知の制約・注意点
- 長時間/大量出力コマンドはライブ送信上限に達する可能性（集約出力で補完）。
- サンドボックス不可環境では `AskUser`/`Reject` が増える（CI 等）。
- Chat Completions 経由では SSE 取り回しが Responses と異なるため挙動差がある（集約で吸収）。
