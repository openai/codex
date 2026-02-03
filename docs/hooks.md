# Hooks 仕様（ドラフト）

## 目的
- TurnStart / TurnEnd のタイミングでコマンドを自動実行する仕組みを提供する。
- Hooks 自体は「コマンド実行を仕組化するだけ」であり、実行結果の扱い（エージェントへの指示化、次Turn起動など）は Codex 側の処理ルールに従う。

## 用語
- TurnStartHook: Turn開始直前に実行される hook
- TurnEndHook: Turn終了直後に実行される hook
- HookInput: Hook の stderr を Codex 側へ渡すための入力形式

## 設定ファイル

### パス
- <workspace>/.codex/hook.toml

### 形式（MVP）
```toml
[turn_start]
command = "path/to/script"
args = ["--foo", "bar"]

[turn_end]
command = "path/to/script"
args = ["--baz"]
```

### 仕様
- command: 実行するコマンド
- args: 引数配列（省略可）
- cwd: ワークスペース固定
- タイムアウトなし（MVP）
- 失敗してもセッションを止めない

## 実行仕様

### TurnStartHook
1. Turn開始前に hook 実行
2. stdout/stderr を ExecCommandBegin/End と StdoutStream 経由で UI へ通知（UserShellCommand と同じリアルタイム表示）
3. stderr が非空なら HookInput として同じ Turn に追加 → ユーザー入力と合成して送信

### TurnEndHook
1. Turn完了直後に hook 実行
2. stdout/stderr を ExecCommandBegin/End と StdoutStream 経由で UI へ通知（UserShellCommand と同じリアルタイム表示）
3. stderr が非空なら HookInput として送信し、新しい Turn を開始

### Sandbox / Approval
- Turn の sandbox / approval 設定に従う

### キャンセル
- Esc による Op::Interrupt と同じ経路で中断可能

## UI 表示

### Hook 実行ログ
- UserShellCommand と同等の見せ方
- タイトル表示例:
  - Hook(TurnStart)
  - Hook(TurnEnd)
- ExecCommandBegin/End と stdout/stderr を表示

### HookInput 表示
- HookInput であることが明示的に分かる表示
- 表示例:
  - HookInput: <stderr text>
- history には残さない

## 履歴・記録
- history: 残さない
- rollout: 残す
  - Hook 実行ログ（ExecCommandBegin/End）
  - HookInput

## HookInput 仕様

### 目的
- Hook の stderr を「フック由来の入力」として Codex に渡す
- ユーザー入力とは区別する

### 実装案
- protocol に HookInput 構造体を追加
- Op::HookInput を追加
- HookInput は UI 表示・rollout記録の対象だが、historyには残さない

## 注意点・設計理由
- stderr はプロセス単位で分離されるため、Hook 実行の stderr が他のコマンド出力と混ざることはない。
- Hook の stderr 内容の制御は hook スクリプト作者の責任とする。

## 非目標（MVP）
- タイムアウト制御
- 非同期実行
- Hook 結果のJSONプロトコル
- Hook 結果による承認/中断の自動化

## テスト方針（MVP）
- core レイヤーのテストで挙動を保障する（TurnStart/TurnEnd の実行と HookInput の注入）
- tui/cli の表示保証は現段階では対象外

## 追加検討（将来拡張）
- HookInput の JSON 形式
- Hook 実行のタイムアウト
- Hook 成否に応じた自動挙動（例: TurnEndで自動停止）
