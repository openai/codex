# Slide CLI 要件定義（MVP）

## 概要 / 目的
- 端末（ターミナル）で実行する最小構成のエージェント型 CLI「Slide」を提供する。
- `slide` コマンドで起動し、会話（チャット）から Markdown スライド（.md）を生成する。
- 初回挙動は既存 Codex CLI に近い UX とし、安全なサンドボックス実行・承認フローを踏襲する（同等で十分）。
- 生成された Markdown をユーザーが任意ツールで PPTX 化することを前提（本MVPでは変換機能は非対象）。

## スコープ（MVP）
- `slide` コマンドの提供（Node 薄いラッパ or 既存 Rust バイナリ起動の別名）。
- 対話（チャット）からスライド要件を受け取り、Markdown ファイルを生成。
- 出力先はリポジトリ直下の `slides/` ディレクトリ（無ければ作成）。
- 承認ポリシー（suggest/auto-edit/full-auto）およびサンドボックス方針は Codex と同等。
- 既存のモデル・プロバイダ設定は流用（モデル選定はデフォルト `o4-mini` 想定）。
- プレビューTUI（最小版）の提供（後述）。

## 非スコープ（MVP）
- PPTX/PDF への自動変換（将来対応）。
- 画像生成・テーマ選択 UI。
- 外部 MCP 連携（MVPでは必須ではない）。

## プレビューTUI（MVP）
- 目的: 生成済み Markdown をターミナルで素早く確認・ページ送りする。
- 技術: Rust ratatui（既存 `codex-rs/tui` のスタイル方針に準拠）。
- 起動方法:
  - `slide preview <path/to/markdown>`
  - または生成直後に「プレビューを開く？」で `y` 選択
- 表示仕様:
  - 左/右でスライド移動（`j/k` もしくは `←/→`）
  - 現在ページ/総ページのステータス表示
  - ANSI スタイルは `ansi-escape` を利用して最低限の装飾レンダ
- 非対応（MVP）:
  - 画像のインライン描画、テーマ切替、ライブ編集反映
- 終了キー: `q`

## ユースケース例
- 「営業向け 10 枚の提案資料を作って」→ 章立て・要点・箇条書きで Markdown 生成。
- 「この仕様書から要約スライド 5 枚」→ 指定ファイルを読み要約して Markdown 生成。
- 「日本語/英語で作って」→ 言語指定で出力。

## CLI 仕様（MVP）
- コマンド名: `slide`
- 動作モード:
  - `slide`（インタラクティブ REPL）
  - `slide "<プロンプト>"`（ワンショット開始）
- 主なフラグ（Codex と共通に揃える）
  - `-a, --approval-mode <suggest|auto-edit|full-auto>`
  - `-m, --model <name>`
  - `-q, --quiet`、`--json`（必要に応じて）
- ファイル生成アクションは承認プロンプトを経由（auto-edit/full-auto では自動）。

## 生成物仕様（Markdown）
- 出力先: `slides/<timestamp>_<slug>.md`
- 推奨フォーマット:
  - 1行目: タイトル（`# タイトル`）
  - スライドは `##` 区切り（1スライド=1セクション）
  - 箇条書き・コード・画像プレースホルダを許容（画像はURL/相対パス）
- 例:
  ```md
  # プロジェクト提案 2025Q1

  ## 1. 課題
  - 現状の問題点
  - 影響範囲

  ## 2. 解決方針
  - 施策A
  - 施策B
  ```

## 初回起動時の挙動
- Codex 同等のオンボーディング（モデル・承認モード・通知など）を簡易にガイド。
- 必要最小限の設定ファイル（`~/.slide/config.(json|yaml)` or 既存 `~/.codex/config.*` を流用）
  - MVPでは Codex の設定ファイルを流用し、`provider/model/approvalMode` を尊重。

## セキュリティ / 実行方針
- Codex と同様にネットワーク無効（Full Auto時も既定は無効）・作業ディレクトリ内に限定。
- Linux ではコンテナ/iptables によるサンドボックスを利用可能（将来互換）。
- ファイル書き込みは承認制（モード依存）。

## ログ / トレース
- `RUST_LOG` / `DEBUG` など既存と同等の出力を踏襲。
- 生成結果（パス）を確実にユーザーへ表示。

## 実装方針（MVP 最短）
1) CLI ラッパ（Node）
   - `slide-cli/` を `codex-cli/` と同構成で作成し、`bin/slide.js` を追加。
   - プラットフォーム判定 → `bin/slide-<target>` を起動（codex と同形式）。
   - 互換最優先のため、Rust 側に `--app slide` などのモードを伝える（環境変数でも可）。

2) Rust 側（codex-rs/cli）
   - `argv0` もしくは `--app slide` で「Slideモード」を有効化。
   - デフォルトのカスタム指示（スライド出力を志向）を適用。
   - ファイル生成アクションのユーティリティ（`slides/` 作成・ファイル命名・上書き確認）。

3) 最小チャット体験
   - REPLで「スライドテーマ/枚数/対象読者/口調/画像有無」等を引き出す簡易プロンプト。
   - 1回目の下書きMDを生成 → 承認 → `slides/` に保存。
   - 再生成/追記/分割などは将来拡張。

## 受け入れ基準（MVP）
- `slide "営業向け提案 10枚 日本語で"` で `slides/*.md` が生成される。
- インタラクティブで追記指示を出すと、上記ファイルに安全に追記（または別名保存）できる。
- suggest/auto-edit/full-auto の各モードで意図どおりに承認フローが機能。

## テスト方針（最小）
- 単体: ファイル命名・スラッグ生成・出力ディレクトリ作成のテスト。
- 統合: 固定プロンプトで Markdown を生成し、想定のヘッダ/セクション数を満たすか検証。

## 今後の拡張
- 画像生成（ダイアグラム/サムネイル）・テーマ（色/フォント）・テンプレート対応。
- PPTX/PDF 自動変換（Pandoc/Marp 等連携の検討）。
- 外部 MCP の利用（社内ナレッジ検索、図表自動作成ツールなど）。

## 未決事項 / 要確認
- パッケージ名（npm）: 仮 `@yourorg/slide`（要確定）。
- 設定ファイル: `~/.slide/*` 新設か `~/.codex/*` 流用か（MVPは流用案）。
- リリース戦略: codex と同一トリプル/同梱方式で問題ないか。

## 想定ディレクトリ構造（MVP・省略なし・MCP除外）
```
repo-root/
├─ slide-cli/                          # Slide 用 Node ランチャ（配布・エントリ）
│  ├─ package.json                     # bin に slide.js を登録
│  ├─ bin/
│  │  └─ slide.js                      # プラットフォーム判定→ slide-<target> 起動
│  └─ scripts/
│     └─ README.md                     # （将来の配布補助。MVPは空で可）
├─ slide-rs/                           # Rust ワークスペース（Codex相当）
│  ├─ Cargo.toml                       # [workspace] members 定義
│  ├─ Cargo.lock
│  ├─ clippy.toml
│  ├─ rust-toolchain.toml
│  ├─ rustfmt.toml
│  ├─ justfile
│  ├─ docs/
│  │  └─ protocol_v1.md               # 必要に応じ参照（そのまま）
│  ├─ scripts/
│  │  └─ create_github_release.sh
│  │
│  ├─ ansi-escape/                     # ANSI→ratatui Text/Line 変換（既存と同等）
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     └─ lib.rs
│  │
│  ├─ apply-patch/                     # 安全な差分適用（エージェント編集の基盤）
│  │  ├─ Cargo.toml
│  │  ├─ src/
│  │  │  ├─ lib.rs
│  │  │  ├─ main.rs
│  │  │  ├─ parser.rs
│  │  │  ├─ seek_sequence.rs
│  │  │  └─ standalone_executable.rs
│  │  └─ tests/
│  │     ├─ all.rs
│  │     └─ suite/
│  │
│  ├─ arg0/                            # argv0 ディスパッチ（単一バイナリ多役割）
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     └─ lib.rs
│  │
│  ├─ chatgpt/                         # OpenAI プロバイダ統合（スライド生成用）
│  │  ├─ Cargo.toml
│  │  ├─ src/
│  │  │  ├─ apply_command.rs
│  │  │  ├─ chatgpt_client.rs
│  │  │  ├─ chatgpt_token.rs
│  │  │  ├─ get_task.rs
│  │  │  └─ lib.rs
│  │  └─ tests/
│  │     ├─ all.rs
│  │     └─ suite/
│  │
│  ├─ cli/                             # Rust CLI 本体（オンボーディングTUI/REPL起点）
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     ├─ debug_sandbox.rs
│  │     ├─ exit_status.rs
│  │     ├─ lib.rs
│  │     ├─ login.rs                   # 認証UIが不要なら後で最小化
│  │     ├─ main.rs
│  │     └─ proto.rs
│  │
│  ├─ common/                          # 共通ユーティリティ（承認モード/設定等）
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     ├─ approval_mode_cli_arg.rs
│  │     ├─ approval_presets.rs
│  │     ├─ config_override.rs
│  │     ├─ config_summary.rs
│  │     ├─ elapsed.rs
│  │     ├─ fuzzy_match.rs
│  │     ├─ lib.rs
│  │     ├─ model_presets.rs
│  │     ├─ sandbox_mode_cli_arg.rs
│  │     └─ sandbox_summary.rs
│  │
│  ├─ core/                            # エージェント中核（チャット/実行/安全性）
│  │  ├─ Cargo.toml
│  │  ├─ README.md
│  │  ├─ prompt.md
│  │  └─ src/
│  │     ├─ apply_patch.rs
│  │     ├─ bash.rs
│  │     ├─ chat_completions.rs        # スライド生成プロンプト処理をここに実装
│  │     ├─ client_common.rs
│  │     ├─ client.rs
│  │     ├─ codex.rs                   # （名称は slide.rs に改称可）
│  │     ├─ codex_conversation.rs
│  │     ├─ config.rs
│  │     ├─ …（既存の安全系・履歴系など必要最小限を残す）
│  │
│  ├─ exec/                            # 非対話実行・イベント処理（必要最小）
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     ├─ cli.rs
│  │     ├─ event_processor.rs
│  │     ├─ lib.rs
│  │     └─ main.rs
│  │
│  ├─ execpolicy/                      # 実行ポリシー（Seatbelt/Landlock 連携）
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     ├─ default.policy
│  │     ├─ lib.rs
│  │     └─ …
│  │
│  ├─ file-search/                     # 依頼文からの検索ユーティリティ（任意）
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     ├─ cli.rs
│  │     ├─ lib.rs
│  │     └─ main.rs
│  │
│  ├─ linux-sandbox/                   # Linux サンドボックス（任意/将来互換）
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     ├─ landlock.rs
│  │     ├─ lib.rs
│  │     ├─ linux_run_main.rs
│  │     └─ main.rs
│  │
│  ├─ protocol/                        # 内部プロトコル（必要箇所のみ利用）
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     └─ …
│  │
│  └─ tui/                             # オンボーディング/チャット/プレビューTUI
│     ├─ Cargo.toml
│     ├─ prompt_for_init_command.md
│     ├─ styles.md
│     └─ src/
│        ├─ app.rs                     # 画面遷移（Onboarding→Chat→Preview）
│        ├─ onboarding.rs              # 初回設定（モデル/承認/通知）
│        ├─ chat.rs                    # チャットUI（要件収集→生成）
│        ├─ preview.rs                 # Markdownページング表示
│        └─ …
│
├─ docs/
│  └─ slide-cli-requirements.md        # この要件定義
├─ slides/
│  └─ sample.md                        # 生成物サンプル
├─ .gitignore
├─ LICENSE
└─ README.md
```

- 明確に除外（本MVPでは持たない）
  - MCP関連（mcp-client / mcp-server / mcp-types）
  - protocol-ts（TSバインディング生成）
  - login（外部ブラウザ連携が不要なら省略可）
  - ollama 等の追加プロバイダ（必要時に追加）

- 重要: オンボーディングTUI→チャット→スライド生成→プレビューTUIの流れを `tui/` に実装。モデル呼び出しは `core/chat_completions.rs` + `chatgpt/` 経由で実施。

## 実装しない機能（MVP明確化）
- PPTX/PDF への自動変換（手動で別ツールに渡す前提）
- テーマ選択・カスタム配色・フォント変更
- スライドテンプレート（高度なテンプレ）
- 画像の自動生成・埋め込み（URLプレースホルダは許容）
- 外部 MCP 連携（社内検索・SaaS連携など）
- リアルタイム共同編集（ライブコラボ）
- 自動要約の高精度評価/スコアリング（簡易生成のみ）
- TUIでのライブ編集反映（MVPは固定表示 + ページング）

## 動作フロー（エンドツーエンド）
1) インストール
   - npm 経由で `slide` を導入（`@yourorg/slide` 想定）。
   - 初回起動でモデル/APIキーなどを案内（Codex同等の簡易ガイド）。

2) 起動モード
   - 対話開始: `slide`
   - ワンショット: `slide "<要件を1行で>"`

3) 対話（チャット）
   - モデルへ質問: テーマ/枚数/対象/口調/画像有無/言語など
   - 合意内容からスライド素案を生成（Markdownテキスト）

4) 承認と保存
   - ファイル内容を表示→承認
   - `slides/<timestamp>_<slug>.md` を作成（既存と衝突する場合は別名）

5) プレビュー（任意）
   - `slide preview slides/<file>.md` でTUI起動
   - ←/→（j/k）でページ遷移、`q` で終了

6) 後続（ユーザー側）
   - 生成Markdownを任意ツールで PPTX/PDF へ変換
   - 必要なら再度 `slide` で修正案を相談→追記/再生成

7) 失敗時の扱い
   - 書き込み失敗や生成不十分な場合は差分提示・再試行をガイド
   - Quiet/JSON モードでは機械可読な結果を返す

## 実装コード（計画反映）

### slide-cli/package.json
```json
{
  "name": "@yourorg/slide",
  "version": "0.0.1",
  "license": "Apache-2.0",
  "bin": {
    "slide": "bin/slide.js"
  },
  "type": "module",
  "engines": {
    "node": ">=20"
  },
  "files": [
    "bin",
    "dist"
  ],
  "dependencies": {
    "@vscode/ripgrep": "^1.15.14"
  },
  "devDependencies": {
    "prettier": "^3.3.3"
  }
}
```

### slide-cli/bin/slide.js
```javascript
#!/usr/bin/env node
// Slide CLI launcher: selects platform-specific native binary and runs it.

import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const { platform, arch } = process;

let targetTriple = null;
switch (platform) {
  case "linux":
  case "android":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-unknown-linux-musl";
        break;
      case "arm64":
        targetTriple = "aarch64-unknown-linux-musl";
        break;
      default:
        break;
    }
    break;
  case "darwin":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-apple-darwin";
        break;
      case "arm64":
        targetTriple = "aarch64-apple-darwin";
        break;
      default:
        break;
    }
    break;
  case "win32":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-pc-windows-msvc.exe";
        break;
      default:
        break;
    }
    break;
  default:
    break;
}

if (!targetTriple) {
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}

const binaryPath = path.join(__dirname, "..", "bin", `slide-${targetTriple}`);

const { spawn } = await import("child_process");

async function tryImport(moduleName) {
  try {
    return await import(moduleName);
  } catch {
    return null;
  }
}

async function resolveRgDir() {
  const ripgrep = await tryImport("@vscode/ripgrep");
  if (!ripgrep?.rgPath) return null;
  return path.dirname(ripgrep.rgPath);
}

function getUpdatedPath(newDirs) {
  const pathSep = process.platform === "win32" ? ";" : ":";
  const existingPath = process.env.PATH || "";
  return [...newDirs, ...existingPath.split(pathSep).filter(Boolean)].join(pathSep);
}

const additionalDirs = [];
const rgDir = await resolveRgDir();
if (rgDir) additionalDirs.push(rgDir);
const updatedPath = getUpdatedPath(additionalDirs);

// Pass a hint so the native binary can switch to Slide mode if shared
const env = { ...process.env, PATH: updatedPath, SLIDE_APP: "1" };

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env,
});

child.on("error", (err) => {
  console.error(err);
  process.exit(1);
});

const forwardSignal = (signal) => {
  if (child.killed) return;
  try { child.kill(signal); } catch {}
};

["SIGINT", "SIGTERM", "SIGHUP"].forEach((sig) => {
  process.on(sig, () => forwardSignal(sig));
});

const childResult = await new Promise((resolve) => {
  child.on("exit", (code, signal) => {
    if (signal) resolve({ type: "signal", signal });
    else resolve({ type: "code", exitCode: code ?? 1 });
  });
});

if (childResult.type === "signal") {
  process.kill(process.pid, childResult.signal);
} else {
  process.exit(childResult.exitCode);
}
```

### slide-cli/README.md
```markdown
# Slide CLI (MVP)

Lightweight terminal agent to generate Markdown slides via chat. Ships as a Node launcher that executes platform-specific native binaries.

## Quickstart
```
npm i -g @yourorg/slide
slide
slide "営業向け提案 10枚 日本語で"
```

## Preview TUI (MVP)
```
slide preview slides/<file>.md
```

- Navigate: ←/→ (or j/k)
- Quit: q

## Notes
- Generates Markdown into `slides/` as `<timestamp>_<slug>.md`.
- PPTX/PDF conversion is out of scope for MVP (use external tools).
```

### slide-cli/scripts/README.md
```markdown
# scripts (optional for MVP)

This directory is reserved for release and sandbox helpers (e.g., staging native binaries).
For the MVP, no scripts are required. Keep this as a placeholder for future tooling.
```

### slides/sample.md
```markdown
# デモスライド：Slide CLI MVP

## 1. 目的
- ターミナルからチャットでスライドを生成
- Markdown を生成し、ユーザーが任意ツールで変換

## 2. スコープ（MVP）
- `slide` コマンド
- チャット→Markdown 生成
- 簡易プレビューTUI（ページ送り）

## 3. 次の一歩
- テンプレート/テーマ対応
- 画像サポート
- PPTX/PDF 変換連携
```

### 参考: 現行実装（変更不要のまま引用）

#### codex-cli/bin/codex.js
```javascript
#!/usr/bin/env node
// Unified entry point for the Codex CLI.

import path from "path";
import { fileURLToPath } from "url";

// __dirname equivalent in ESM
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const { platform, arch } = process;

let targetTriple = null;
switch (platform) {
  case "linux":
  case "android":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-unknown-linux-musl";
        break;
      case "arm64":
        targetTriple = "aarch64-unknown-linux-musl";
        break;
      default:
        break;
    }
    break;
  case "darwin":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-apple-darwin";
        break;
      case "arm64":
        targetTriple = "aarch64-apple-darwin";
        break;
      default:
        break;
    }
    break;
  case "win32":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-pc-windows-msvc.exe";
        break;
      case "arm64":
      // We do not build this today, fall through...
      default:
        break;
    }
    break;
  default:
    break;
}

if (!targetTriple) {
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}

const binaryPath = path.join(__dirname, "..", "bin", `codex-${targetTriple}`);

// Use an asynchronous spawn instead of spawnSync so that Node is able to
// respond to signals (e.g. Ctrl-C / SIGINT) while the native binary is
// executing. This allows us to forward those signals to the child process
// and guarantees that when either the child terminates or the parent
// receives a fatal signal, both processes exit in a predictable manner.
const { spawn } = await import("child_process");

async function tryImport(moduleName) {
  try {
    // eslint-disable-next-line node/no-unsupported-features/es-syntax
    return await import(moduleName);
  } catch (err) {
    return null;
  }
}

async function resolveRgDir() {
  const ripgrep = await tryImport("@vscode/ripgrep");
  if (!ripgrep?.rgPath) {
    return null;
  }
  return path.dirname(ripgrep.rgPath);
}

function getUpdatedPath(newDirs) {
  const pathSep = process.platform === "win32" ? ";" : ":";
  const existingPath = process.env.PATH || "";
  const updatedPath = [
    ...newDirs,
    ...existingPath.split(pathSep).filter(Boolean),
  ].join(pathSep);
  return updatedPath;
}

const additionalDirs = [];
const rgDir = await resolveRgDir();
if (rgDir) {
  additionalDirs.push(rgDir);
}
const updatedPath = getUpdatedPath(additionalDirs);

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env: { ...process.env, PATH: updatedPath, CODEX_MANAGED_BY_NPM: "1" },
});

child.on("error", (err) => {
  // Typically triggered when the binary is missing or not executable.
  // Re-throwing here will terminate the parent with a non-zero exit code
  // while still printing a helpful stack trace.
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});

// Forward common termination signals to the child so that it shuts down
// gracefully. In the handler we temporarily disable the default behavior of
// exiting immediately; once the child has been signaled we simply wait for
// its exit event which will in turn terminate the parent (see below).
const forwardSignal = (signal) => {
  if (child.killed) {
    return;
  }
  try {
    child.kill(signal);
  } catch {
    /* ignore */
  }
};

["SIGINT", "SIGTERM", "SIGHUP"].forEach((sig) => {
  process.on(sig, () => forwardSignal(sig));
});

// When the child exits, mirror its termination reason in the parent so that
// shell scripts and other tooling observe the correct exit status.
// Wrap the lifetime of the child process in a Promise so that we can await
// its termination in a structured way. The Promise resolves with an object
// describing how the child exited: either via exit code or due to a signal.
const childResult = await new Promise((resolve) => {
  child.on("exit", (code, signal) => {
    if (signal) {
      resolve({ type: "signal", signal });
    } else {
      resolve({ type: "code", exitCode: code ?? 1 });
    }
  });
});

if (childResult.type === "signal") {
  // Re-emit the same signal so that the parent terminates with the expected
  // semantics (this also sets the correct exit code of 128 + n).
  process.kill(process.pid, childResult.signal);
} else {
  process.exit(childResult.exitCode);
}
```

#### codex-rs/cli/src/main.rs
```rust
use clap::CommandFactory;
use clap::Parser;
use clap_complete::Shell;
use clap_complete::generate;
use codex_arg0::arg0_dispatch_or_else;
use codex_chatgpt::apply_command::ApplyCommand;
use codex_chatgpt::apply_command::run_apply_command;
use codex_cli::LandlockCommand;
use codex_cli::SeatbeltCommand;
use codex_cli::login::run_login_status;
use codex_cli::login::run_login_with_api_key;
use codex_cli::login::run_login_with_chatgpt;
use codex_cli::login::run_logout;
use codex_cli::proto;
use codex_common::CliConfigOverrides;
use codex_exec::Cli as ExecCli;
use codex_tui::Cli as TuiCli;
use std::path::PathBuf;

use crate::proto::ProtoCli;

/// Codex CLI
///
/// If no subcommand is specified, options will be forwarded to the interactive CLI.
#[derive(Debug, Parser)]
#[clap(
    author,
    version,
    // If a sub‑command is given, ignore requirements of the default args.
    subcommand_negates_reqs = true,
    // The executable is sometimes invoked via a platform‑specific name like
    // `codex-x86_64-unknown-linux-musl`, but the help output should always use
    // the generic `codex` command name that users run.
    bin_name = "codex"
)]
struct MultitoolCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    interactive: TuiCli,

    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Run Codex non-interactively.
    #[clap(visible_alias = "e")]
    Exec(ExecCli),

    /// Manage login.
    Login(LoginCommand),

    /// Remove stored authentication credentials.
    Logout(LogoutCommand),

    /// Experimental: run Codex as an MCP server.
    Mcp,

    /// Run the Protocol stream via stdin/stdout
    #[clap(visible_alias = "p")]
    Proto(ProtoCli),

    /// Generate shell completion scripts.
    Completion(CompletionCommand),

    /// Internal debugging commands.
    Debug(DebugArgs),

    /// Apply the latest diff produced by Codex agent as a `git apply` to your local working tree.
    #[clap(visible_alias = "a")]
    Apply(ApplyCommand),

    /// Internal: generate TypeScript protocol bindings.
    #[clap(hide = true)]
    GenerateTs(GenerateTsCommand),
}

#[derive(Debug, Parser)]
struct CompletionCommand {
    /// Shell to generate completions for
    #[clap(value_enum, default_value_t = Shell::Bash)]
    shell: Shell,
}

#[derive(Debug, Parser)]
struct DebugArgs {
    #[command(subcommand)]
    cmd: DebugCommand,
}

#[derive(Debug, clap::Subcommand)]
enum DebugCommand {
    /// Run a command under Seatbelt (macOS only).
    Seatbelt(SeatbeltCommand),

    /// Run a command under Landlock+seccomp (Linux only).
    Landlock(LandlockCommand),
}

#[derive(Debug, Parser)]
struct LoginCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,

    #[arg(long = "api-key", value_name = "API_KEY")]
    api_key: Option<String>,

    #[command(subcommand)]
    action: Option<LoginSubcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum LoginSubcommand {
    /// Show login status.
    Status,
}

#[derive(Debug, Parser)]
struct LogoutCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,
}

#[derive(Debug, Parser)]
struct GenerateTsCommand {
    /// Output directory where .ts files will be written
    #[arg(short = 'o', long = "out", value_name = "DIR")]
    out_dir: PathBuf,

    /// Optional path to the Prettier executable to format generated files
    #[arg(short = 'p', long = "prettier", value_name = "PRETTIER_BIN")]
    prettier: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        cli_main(codex_linux_sandbox_exe).await?;
        Ok(())
    })
}

async fn cli_main(codex_linux_sandbox_exe: Option<PathBuf>) -> anyhow::Result<()> {
    let cli = MultitoolCli::parse();

    match cli.subcommand {
        None => {
            let mut tui_cli = cli.interactive;
            prepend_config_flags(&mut tui_cli.config_overrides, cli.config_overrides);
            let usage = codex_tui::run_main(tui_cli, codex_linux_sandbox_exe).await?;
            if !usage.is_zero() {
                println!("{}", codex_core::protocol::FinalOutput::from(usage));
            }
        }
        Some(Subcommand::Exec(mut exec_cli)) => {
            prepend_config_flags(&mut exec_cli.config_overrides, cli.config_overrides);
            codex_exec::run_main(exec_cli, codex_linux_sandbox_exe).await?;
        }
        Some(Subcommand::Mcp) => {
            codex_mcp_server::run_main(codex_linux_sandbox_exe, cli.config_overrides).await?;
        }
        Some(Subcommand::Login(mut login_cli)) => {
            prepend_config_flags(&mut login_cli.config_overrides, cli.config_overrides);
            match login_cli.action {
                Some(LoginSubcommand::Status) => {
                    run_login_status(login_cli.config_overrides).await;
                }
                None => {
                    if let Some(api_key) = login_cli.api_key {
                        run_login_with_api_key(login_cli.config_overrides, api_key).await;
                    } else {
                        run_login_with_chatgpt(login_cli.config_overrides).await;
                    }
                }
            }
        }
        Some(Subcommand::Logout(mut logout_cli)) => {
            prepend_config_flags(&mut logout_cli.config_overrides, cli.config_overrides);
            run_logout(logout_cli.config_overrides).await;
        }
        Some(Subcommand::Proto(mut proto_cli)) => {
            prepend_config_flags(&mut proto_cli.config_overrides, cli.config_overrides);
            proto::run_main(proto_cli).await?;
        }
        Some(Subcommand::Completion(completion_cli)) => {
            print_completion(completion_cli);
        }
        Some(Subcommand::Debug(debug_args)) => match debug_args.cmd {
            DebugCommand::Seatbelt(mut seatbelt_cli) => {
                prepend_config_flags(&mut seatbelt_cli.config_overrides, cli.config_overrides);
                codex_cli::debug_sandbox::run_command_under_seatbelt(
                    seatbelt_cli,
                    codex_linux_sandbox_exe,
                )
                .await?;
            }
            DebugCommand::Landlock(mut landlock_cli) => {
                prepend_config_flags(&mut landlock_cli.config_overrides, cli.config_overrides);
                codex_cli::debug_sandbox::run_command_under_landlock(
                    landlock_cli,
                    codex_linux_sandbox_exe,
                )
                .await?;
            }
        },
        Some(Subcommand::Apply(mut apply_cli)) => {
            prepend_config_flags(&mut apply_cli.config_overrides, cli.config_overrides);
            run_apply_command(apply_cli, None).await?;
        }
        Some(Subcommand::GenerateTs(gen_cli)) => {
            codex_protocol_ts::generate_ts(&gen_cli.out_dir, gen_cli.prettier.as_deref())?;
        }
    }

    Ok(())
}

/// Prepend root-level overrides so they have lower precedence than
/// CLI-specific ones specified after the subcommand (if any).
fn prepend_config_flags(
    subcommand_config_overrides: &mut CliConfigOverrides,
    cli_config_overrides: CliConfigOverrides,
) {
    subcommand_config_overrides
        .raw_overrides
        .splice(0..0, cli_config_overrides.raw_overrides);
}

fn print_completion(cmd: CompletionCommand) {
    let mut app = MultitoolCli::command();
    let name = "codex";
    generate(cmd.shell, &mut app, name, &mut std::io::stdout());
}
```

#### codex-rs/ansi-escape/Cargo.toml
```toml
[package]
edition = "2024"
name = "codex-ansi-escape"
version = { workspace = true }

[lib]
name = "codex_ansi_escape"
path = "src/lib.rs"

[dependencies]
ansi-to-tui = "7.0.0"
ratatui = { version = "0.29.0", features = [
    "unstable-rendered-line-info",
    "unstable-widget-ref",
] }
tracing = { version = "0.1.41", features = ["log"] }
```

#### codex-rs/ansi-escape/src/lib.rs
```rust
use ansi_to_tui::Error;
use ansi_to_tui::IntoText;
use ratatui::text::Line;
use ratatui::text::Text;

/// This function should be used when the contents of `s` are expected to match
/// a single line. If multiple lines are found, a warning is logged and only the
/// first line is returned.
pub fn ansi_escape_line(s: &str) -> Line<'static> {
    let text = ansi_escape(s);
    match text.lines.as_slice() {
        [] => Line::from(""),
        [only] => only.clone(),
        [first, rest @ ..] => {
            tracing::warn!("ansi_escape_line: expected a single line, got {first:?} and {rest:?}");
            first.clone()
        }
    }
}

pub fn ansi_escape(s: &str) -> Text<'static> {
    // to_text() claims to be faster, but introduces complex lifetime issues
    // such that it's not worth it.
    match s.into_text() {
        Ok(text) => text,
        Err(err) => match err {
            Error::NomError(message) => {
                tracing::error!(
                    "ansi_to_tui NomError docs claim should never happen when parsing `{s}`: {message}"
                );
                panic!();
            }
            Error::Utf8Error(utf8error) => {
                tracing::error!("Utf8Error: {utf8error}");
                panic!();
            }
        },
    }
}
```

#### codex-cli/package.json
```json
{
  "name": "@openai/codex",
  "version": "0.0.0-dev",
  "license": "Apache-2.0",
  "bin": {
    "codex": "bin/codex.js"
  },
  "type": "module",
  "engines": {
    "node": ">=20"
  },
  "files": [
    "bin",
    "dist"
  ],
  "repository": {
    "type": "git",
    "url": "git+https://github.com/openai/codex.git"
  },
  "dependencies": {
    "@vscode/ripgrep": "^1.15.14"
  },
  "devDependencies": {
    "prettier": "^3.3.3"
  }
}
```

#### codex-cli/README.md
```markdown
(here follows the full README as in repo)
```

#### codex-cli/Dockerfile
```dockerfile
FROM node:24-slim

ARG TZ
ENV TZ="$TZ"

# Install basic development tools, ca-certificates, and iptables/ipset, then clean up apt cache to reduce image size
RUN apt-get update && apt-get install -y --no-install-recommends \
  aggregate \
  ca-certificates \
  curl \
  dnsutils \
  fzf \
  gh \
  git \
  gnupg2 \
  iproute2 \
  ipset \
  iptables \
  jq \
  less \
  man-db \
  procps \
  unzip \
  ripgrep \
  zsh \
  && rm -rf /var/lib/apt/lists/*

# Ensure default node user has access to /usr/local/share
RUN mkdir -p /usr/local/share/npm-global && \
  chown -R node:node /usr/local/share

ARG USERNAME=node

# Set up non-root user
USER node

# Install global packages
ENV NPM_CONFIG_PREFIX=/usr/local/share/npm-global
ENV PATH=$PATH:/usr/local/share/npm-global/bin

# Install codex
COPY dist/codex.tgz codex.tgz
RUN npm install -g codex.tgz \
  && npm cache clean --force \
  && rm -rf /usr/local/share/npm-global/lib/node_modules/codex-cli/node_modules/.cache \
  && rm -rf /usr/local/share/npm-global/lib/node_modules/codex-cli/tests \
  && rm -rf /usr/local/share/npm-global/lib/node_modules/codex-cli/docs

# Inside the container we consider the environment already sufficiently locked
# down, therefore instruct Codex CLI to allow running without sandboxing.
ENV CODEX_UNSAFE_ALLOW_NO_SANDBOX=1

# Copy and set up firewall script as root.
USER root
COPY scripts/init_firewall.sh /usr/local/bin/
RUN chmod 500 /usr/local/bin/init_firewall.sh

# Drop back to non-root.
USER node
```

#### codex-cli/scripts/build_container.sh
```bash
#!/bin/bash

set -euo pipefail

SCRIPT_DIR=$(realpath "$(dirname "$0")")
trap "popd >> /dev/null" EXIT
pushd "$SCRIPT_DIR/.." >> /dev/null || {
  echo "Error: Failed to change directory to $SCRIPT_DIR/.."
  exit 1
}
pnpm install
pnpm run build
rm -rf ./dist/openai-codex-*.tgz
pnpm pack --pack-destination ./dist
mv ./dist/openai-codex-*.tgz ./dist/codex.tgz
docker build -t codex -f "./Dockerfile" .
```

#### codex-cli/scripts/init_firewall.sh
```bash
#!/bin/bash
set -euo pipefail  # Exit on error, undefined vars, and pipeline failures
IFS=$'\n\t'       # Stricter word splitting

# Read allowed domains from file
ALLOWED_DOMAINS_FILE="/etc/codex/allowed_domains.txt"
if [ -f "$ALLOWED_DOMAINS_FILE" ]; then
    ALLOWED_DOMAINS=()
    while IFS= read -r domain; do
        ALLOWED_DOMAINS+=("$domain")
    done < "$ALLOWED_DOMAINS_FILE"
    echo "Using domains from file: ${ALLOWED_DOMAINS[*]}"
else
    # Fallback to default domains
    ALLOWED_DOMAINS=("api.openai.com")
    echo "Domains file not found, using default: ${ALLOWED_DOMAINS[*]}"
fi

# Ensure we have at least one domain
if [ ${#ALLOWED_DOMAINS[@]} -eq 0 ]; then
    echo "ERROR: No allowed domains specified"
    exit 1
fi

# Flush existing rules and delete existing ipsets
iptables -F
iptables -X
iptables -t nat -F
iptables -t nat -X
iptables -t mangle -F
iptables -t mangle -X
ipset destroy allowed-domains 2>/dev/null || true

# First allow DNS and localhost before any restrictions
# Allow outbound DNS
iptables -A OUTPUT -p udp --dport 53 -j ACCEPT
# Allow inbound DNS responses
iptables -A INPUT -p udp --sport 53 -j ACCEPT
# Allow localhost
iptables -A INPUT -i lo -j ACCEPT
iptables -A OUTPUT -o lo -j ACCEPT

# Create ipset with CIDR support
ipset create allowed-domains hash:net

# Resolve and add other allowed domains
for domain in "${ALLOWED_DOMAINS[@]}"; do
    echo "Resolving $domain..."
    ips=$(dig +short A "$domain")
    if [ -z "$ips" ]; then
        echo "ERROR: Failed to resolve $domain"
        exit 1
    fi

    while read -r ip; do
        if [[ ! "$ip" =~ ^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$ ]]; then
            echo "ERROR: Invalid IP from DNS for $domain: $ip"
            exit 1
        fi
        echo "Adding $ip for $domain"
        ipset add allowed-domains "$ip"
    done < <(echo "$ips")
done

# Get host IP from default route
HOST_IP=$(ip route | grep default | cut -d" " -f3)
if [ -z "$HOST_IP" ]; then
    echo "ERROR: Failed to detect host IP"
    exit 1
fi

HOST_NETWORK=$(echo "$HOST_IP" | sed "s/\.[0-9]*$/.0\/24/")
echo "Host network detected as: $HOST_NETWORK"

# Set up remaining iptables rules
iptables -A INPUT -s "$HOST_NETWORK" -j ACCEPT
iptables -A OUTPUT -d "$HOST_NETWORK" -j ACCEPT

# Set default policies to DROP first
iptables -P INPUT DROP
iptables -P FORWARD DROP
iptables -P OUTPUT DROP

# First allow established connections for already approved traffic
iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
iptables -A OUTPUT -m state --state ESTABLISHED,RELATED -j ACCEPT

# Then allow only specific outbound traffic to allowed domains
iptables -A OUTPUT -m set --match-set allowed-domains dst -j ACCEPT

# Append final REJECT rules for immediate error responses
# For TCP traffic, send a TCP reset; for UDP, send ICMP port unreachable.
iptables -A INPUT -p tcp -j REJECT --reject-with tcp-reset
iptables -A INPUT -p udp -j REJECT --reject-with icmp-port-unreachable
iptables -A OUTPUT -p tcp -j REJECT --reject-with tcp-reset
iptables -A OUTPUT -p udp -j REJECT --reject-with icmp-port-unreachable
iptables -A FORWARD -p tcp -j REJECT --reject-with tcp-reset
iptables -A FORWARD -p udp -j REJECT --reject-with icmp-port-unreachable

echo "Firewall configuration complete"
echo "Verifying firewall rules..."
if curl --connect-timeout 5 https://example.com >/dev/null 2>&1; then
    echo "ERROR: Firewall verification failed - was able to reach https://example.com"
    exit 1
else
    echo "Firewall verification passed - unable to reach https://example.com as expected"
fi

# Always verify OpenAI API access is working
if ! curl --connect-timeout 5 https://api.openai.com >/dev/null 2>&1; then
    echo "ERROR: Firewall verification failed - unable to reach https://api.openai.com"
    exit 1
else
    echo "Firewall verification passed - able to reach https://api.openai.com as expected"
fi
```

#### codex-cli/scripts/install_native_deps.sh
```bash
#!/usr/bin/env bash

# Install native runtime dependencies for codex-cli.
#
# Usage
#   install_native_deps.sh [--workflow-url URL] [CODEX_CLI_ROOT]
#
# The optional RELEASE_ROOT is the path that contains package.json.  Omitting
# it installs the binaries into the repository's own bin/ folder to support
# local development.

set -euo pipefail

# ------------------
# Parse arguments
# ------------------

CODEX_CLI_ROOT=""

# Until we start publishing stable GitHub releases, we have to grab the binaries
# from the GitHub Action that created them. Update the URL below to point to the
# appropriate workflow run:
WORKFLOW_URL="https://github.com/openai/codex/actions/runs/16840150768" # rust-v0.20.0-alpha.2

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workflow-url)
      shift || { echo "--workflow-url requires an argument"; exit 1; }
      if [ -n "$1" ]; then
        WORKFLOW_URL="$1"
      fi
      ;;
    *)
      if [[ -z "$CODEX_CLI_ROOT" ]]; then
        CODEX_CLI_ROOT="$1"
      else
        echo "Unexpected argument: $1" >&2
        exit 1
      fi
      ;;
  esac
  shift

done

# ----------------------------------------------------------------------------
# Determine where the binaries should be installed.
# ----------------------------------------------------------------------------

if [ -n "$CODEX_CLI_ROOT" ]; then
  # The caller supplied a release root directory.
  BIN_DIR="$CODEX_CLI_ROOT/bin"
else
  # No argument; fall back to the repo’s own bin directory.
  # Resolve the path of this script, then walk up to the repo root.
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  CODEX_CLI_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
  BIN_DIR="$CODEX_CLI_ROOT/bin"
fi

# Make sure the destination directory exists.
mkdir -p "$BIN_DIR"

# ----------------------------------------------------------------------------
# Download and decompress the artifacts from the GitHub Actions workflow.
# ----------------------------------------------------------------------------

WORKFLOW_ID="${WORKFLOW_URL##*/}"

ARTIFACTS_DIR="$(mktemp -d)"
trap 'rm -rf "$ARTIFACTS_DIR"' EXIT

# NB: The GitHub CLI `gh` must be installed and authenticated.
gh run download --dir "$ARTIFACTS_DIR" --repo openai/codex "$WORKFLOW_ID"

# x64 Linux
zstd -d "$ARTIFACTS_DIR/x86_64-unknown-linux-musl/codex-x86_64-unknown-linux-musl.zst" \
    -o "$BIN_DIR/codex-x86_64-unknown-linux-musl"
# ARM64 Linux
zstd -d "$ARTIFACTS_DIR/aarch64-unknown-linux-musl/codex-aarch64-unknown-linux-musl.zst" \
    -o "$BIN_DIR/codex-aarch64-unknown-linux-musl"
# x64 macOS
zstd -d "$ARTIFACTS_DIR/x86_64-apple-darwin/codex-x86_64-apple-darwin.zst" \
    -o "$BIN_DIR/codex-x86_64-apple-darwin"
# ARM64 macOS
zstd -d "$ARTIFACTS_DIR/aarch64-apple-darwin/codex-aarch64-apple-darwin.zst" \
    -o "$BIN_DIR/codex-aarch64-apple-darwin"
# x64 Windows
zstd -d "$ARTIFACTS_DIR/x86_64-pc-windows-msvc/codex-x86_64-pc-windows-msvc.exe.zst" \
    -o "$BIN_DIR/codex-x86_64-pc-windows-msvc.exe"

echo "Installed native dependencies into $BIN_DIR"
```

#### codex-cli/scripts/run_in_container.sh
```bash
#!/bin/bash
set -e

# Usage:
#   ./run_in_container.sh [--work_dir directory] "COMMAND"
#
#   Examples:
#     ./run_in_container.sh --work_dir project/code "ls -la"
#     ./run_in_container.sh "echo Hello, world!"

# Default the work directory to WORKSPACE_ROOT_DIR if not provided.
WORK_DIR="${WORKSPACE_ROOT_DIR:-$(pwd)}"
# Default allowed domains - can be overridden with OPENAI_ALLOWED_DOMAINS env var
OPENAI_ALLOWED_DOMAINS="${OPENAI_ALLOWED_DOMAINS:-api.openai.com}"

# Parse optional flag.
if [ "$1" = "--work_dir" ]; then
  if [ -z "$2" ]; then
    echo "Error: --work_dir flag provided but no directory specified."
    exit 1
  fi
  WORK_DIR="$2"
  shift 2
fi

WORK_DIR=$(realpath "$WORK_DIR")

# Generate a unique container name based on the normalized work directory
CONTAINER_NAME="codex_$(echo "$WORK_DIR" | sed 's/\//_/g' | sed 's/[^a-zA-Z0-9_-]//g')"

# Define cleanup to remove the container on script exit, ensuring no leftover containers
cleanup() {
  docker rm -f "$CONTAINER_NAME" >/dev/null 2>&1 || true
}
# Trap EXIT to invoke cleanup regardless of how the script terminates
trap cleanup EXIT

# Ensure a command is provided.
if [ "$#" -eq 0 ]; then
  echo "Usage: $0 [--work_dir directory] \"COMMAND\""
  exit 1
fi

# Check if WORK_DIR is set.
if [ -z "$WORK_DIR" ]; then
  echo "Error: No work directory provided and WORKSPACE_ROOT_DIR is not set."
  exit 1
fi

# Verify that OPENAI_ALLOWED_DOMAINS is not empty
if [ -z "$OPENAI_ALLOWED_DOMAINS" ]; then
  echo "Error: OPENAI_ALLOWED_DOMAINS is empty."
  exit 1
fi

# Kill any existing container for the working directory using cleanup(), centralizing removal logic.
cleanup

# Run the container with the specified directory mounted at the same path inside the container.
docker run --name "$CONTAINER_NAME" -d \
  -e OPENAI_API_KEY \
  --cap-add=NET_ADMIN \
  --cap-add=NET_RAW \
  -v "$WORK_DIR:/app$WORK_DIR" \
  codex \
  sleep infinity

# Write the allowed domains to a file in the container
docker exec --user root "$CONTAINER_NAME" bash -c "mkdir -p /etc/codex"
for domain in $OPENAI_ALLOWED_DOMAINS; do
  # Validate domain format to prevent injection
  if [[ ! "$domain" =~ ^[a-zA-Z0-9][a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$ ]]; then
    echo "Error: Invalid domain format: $domain"
    exit 1
  fi
  echo "$domain" | docker exec --user root -i "$CONTAINER_NAME" bash -c "cat >> /etc/codex/allowed_domains.txt"
done

# Set proper permissions on the domains file
docker exec --user root "$CONTAINER_NAME" bash -c "chmod 444 /etc/codex/allowed_domains.txt && chown root:root /etc/codex/allowed_domains.txt"

# Initialize the firewall inside the container as root user
docker exec --user root "$CONTAINER_NAME" bash -c "/usr/local/bin/init_firewall.sh"

# Remove the firewall script after running it
docker exec --user root "$CONTAINER_NAME" bash -c "rm -f /usr/local/bin/init_firewall.sh"

# Execute the provided command in the container, ensuring it runs in the work directory.
# We use a parameterized bash command to safely handle the command and directory.

quoted_args=""
for arg in "$@"; do
  quoted_args+=" $(printf '%q' "$arg")"
done
docker exec -it "$CONTAINER_NAME" bash -c "cd \"/app$WORK_DIR\" && codex --full-auto ${quoted_args}"
```

#### codex-cli/scripts/stage_release.sh
```bash
#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# stage_release.sh
# -----------------------------------------------------------------------------
# Stages an npm release for @openai/codex.
#
# Usage:
#
#   --tmp <dir>  : Use <dir> instead of a freshly created temp directory.
#   -h|--help    : Print usage.
#
# -----------------------------------------------------------------------------

set -euo pipefail

# Helper - usage / flag parsing

usage() {
  cat <<EOF
Usage: $(basename "$0") [--tmp DIR] [--version VERSION]

Options
  --tmp DIR   Use DIR to stage the release (defaults to a fresh mktemp dir)
  --version   Specify the version to release (defaults to a timestamp-based version)
  -h, --help  Show this help

Legacy positional argument: the first non-flag argument is still interpreted
as the temporary directory (for backwards compatibility) but is deprecated.
EOF
  exit "${1:-0}"
}

TMPDIR=""
# Default to a timestamp-based version (keep same scheme as before)
VERSION="$(printf '0.1.%d' "$(date +%y%m%d%H%M)")"
WORKFLOW_URL=""

# Manual flag parser - Bash getopts does not handle GNU long options well.
while [[ $# -gt 0 ]]; do
  case "$1" in
    --tmp)
      shift || { echo "--tmp requires an argument"; usage 1; }
      TMPDIR="$1"
      ;;
    --tmp=*)
      TMPDIR="${1#*=}"
      ;;
    --version)
      shift || { echo "--version requires an argument"; usage 1; }
      VERSION="$1"
      ;;
    --workflow-url)
      shift || { echo "--workflow-url requires an argument"; exit 1; }
      WORKFLOW_URL="$1"
      ;;
    -h|--help)
      usage 0
      ;;
    --*)
      echo "Unknown option: $1" >&2
      usage 1
      ;;
    *)
      echo "Unexpected extra argument: $1" >&2
      usage 1
      ;;
  esac
  shift

done

# Fallback when the caller did not specify a directory.
# If no directory was specified create a fresh temporary one.
if [[ -z "$TMPDIR" ]]; then
  TMPDIR="$(mktemp -d)"
fi

# Ensure the directory exists, then resolve to an absolute path.
mkdir -p "$TMPDIR"
TMPDIR="$(cd "$TMPDIR" && pwd)"

# Main build logic

echo "Staging release in $TMPDIR"

# The script lives in codex-cli/scripts/ - change into codex-cli root so that
# relative paths keep working.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CODEX_CLI_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

pushd "$CODEX_CLI_ROOT" >/dev/null

# 1. Build the JS artifacts ---------------------------------------------------

# Paths inside the staged package
mkdir -p "$TMPDIR/bin"

cp -r bin/codex.js "$TMPDIR/bin/codex.js"
cp ../README.md "$TMPDIR" || true # README is one level up - ignore if missing

# Modify package.json - bump version and optionally add the native directory to
# the files array so that the binaries are published to npm.

jq --arg version "$VERSION" \
    '.version = $version' \
    package.json > "$TMPDIR/package.json"

# 2. Native runtime deps (sandbox plus optional Rust binaries)

./scripts/install_native_deps.sh --workflow-url "$WORKFLOW_URL" "$TMPDIR"

popd >/dev/null

echo "Staged version $VERSION for release in $TMPDIR"

echo "Verify the CLI:"
echo "    node ${TMPDIR}/bin/codex.js --version"
echo "    node ${TMPDIR}/bin/codex.js --help"

# Print final hint for convenience
echo "Next:  cd \"$TMPDIR\" && npm publish"
```

#### codex-cli/scripts/stage_rust_release.py
```python
#!/usr/bin/env python3

import json
import subprocess
import sys
import argparse
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(
        description="""Stage a release for the npm module.

Run this after the GitHub Release has been created and use
`--release-version` to specify the version to release.

Optionally pass `--tmp` to control the temporary staging directory that will be
forwarded to stage_release.sh.
"""
    )
    parser.add_argument(
        "--release-version", required=True, help="Version to release, e.g., 0.3.0"
    )
    parser.add_argument(
        "--tmp",
        help="Optional path to stage the npm package; forwarded to stage_release.sh",
    )
    args = parser.parse_args()
    version = args.release_version

    gh_run = subprocess.run(
        [
            "gh",
            "run",
            "list",
            "--branch",
            f"rust-v{version}",
            "--json",
            "workflowName,url,headSha",
            "--jq",
            'first(.[] | select(.workflowName == "rust-release"))',
        ],
        stdout=subprocess.PIPE,
        check=True,
    )
    gh_run.check_returncode()
    workflow = json.loads(gh_run.stdout)
    sha = workflow["headSha"]

    print(f"should `git checkout {sha}`")

    current_dir = Path(__file__).parent.resolve()
    cmd = [
        str(current_dir / "stage_release.sh"),
        "--version",
        version,
        "--workflow-url",
        workflow["url"],
    ]
    if args.tmp:
        cmd.extend(["--tmp", args.tmp])

    stage_release = subprocess.run(cmd)
    stage_release.check_returncode()

    return 0


if __name__ == "__main__":
    sys.exit(main())
```

#### codex-cli/scripts/README.md
```markdown
# npm releases

Run the following:

To build the 0.2.x or later version of the npm module, which runs the Rust version of the CLI, build it as follows:

```bash
./codex-cli/scripts/stage_rust_release.py --release-version 0.6.0
```
```

### slide-rs/tui/src/app.rs
```rust
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders},
    Terminal,
};

pub enum AppScreen {
    Onboarding,
    Chat,
    Preview,
}

pub struct AppState {
    pub screen: AppScreen,
}

impl Default for AppState {
    fn default() -> Self {
        Self { screen: AppScreen::Onboarding }
    }
}

pub fn run_app(mut state: AppState) -> anyhow::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(f.size());

            let title = match state.screen {
                AppScreen::Onboarding => "Onboarding",
                AppScreen::Chat => "Chat",
                AppScreen::Preview => "Preview",
            };
            let block = Block::default().title(title).borders(Borders::ALL);
            f.render_widget(block, chunks[0]);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('1') => state.screen = AppScreen::Onboarding,
                    KeyCode::Char('2') => state.screen = AppScreen::Chat,
                    KeyCode::Char('3') => state.screen = AppScreen::Preview,
                    _ => {}
                }
            }
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}

### slide-rs/tui/src/onboarding.rs
```rust
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct OnboardingModel {
    pub selected_model: String,
    pub approval_mode: String,
}

impl Default for OnboardingModel {
    fn default() -> Self {
        Self {
            selected_model: "o4-mini".to_string(),
            approval_mode: "suggest".to_string(),
        }
    }
}

pub fn render_onboarding(f: &mut Frame, area: ratatui::prelude::Rect, model: &OnboardingModel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(100),
        ])
        .split(area);

    let title = format!(
        "Slide Onboarding\nModel: {}  |  Approval: {}\nPress 2 to continue",
        model.selected_model, model.approval_mode
    );

    let p = Paragraph::new(Line::from(title))
        .block(Block::default().title("Onboarding").borders(Borders::ALL))
        .alignment(Alignment::Center)
        .style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(p, layout[0]);
}

### slide-rs/tui/src/chat.rs
```rust
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    text::Text,
    Frame,
};

pub struct ChatState {
    pub history: Vec<String>,
    pub input: String,
}

impl Default for ChatState {
    fn default() -> Self {
        Self { history: vec![], input: String::new() }
    }
}

pub fn render_chat(f: &mut Frame, area: ratatui::prelude::Rect, state: &ChatState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3), // history
            Constraint::Length(3), // input
        ])
        .split(area);

    let history_text = Text::from(state.history.join("\n"));
    let history = Paragraph::new(history_text).block(Block::default().title("Chat").borders(Borders::ALL));
    let input = Paragraph::new(state.input.clone()).block(Block::default().title("Input").borders(Borders::ALL));

    f.render_widget(history, chunks[0]);
    f.render_widget(input, chunks[1]);
}

### slide-rs/tui/src/preview.rs
```rust
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    text::Text,
    Frame,
};

pub struct PreviewState {
    pub slides: Vec<String>,
    pub index: usize,
}

impl PreviewState {
    pub fn current(&self) -> &str { self.slides.get(self.index).map(String::as_str).unwrap_or("") }
}

pub fn render_preview(f: &mut Frame, area: ratatui::prelude::Rect, state: &PreviewState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(100),
        ])
        .split(area);

    let body = Paragraph::new(Text::from(state.current()))
        .block(Block::default().title(format!("Preview {}/{}", state.index + 1, state.slides.len())).borders(Borders::ALL));

    f.render_widget(body, chunks[0]);
}

### slide-rs/core/src/chat_completions.rs
```rust
use anyhow::Result;

pub struct SlideRequest {
    pub title: String,
    pub num_slides: usize,
    pub language: String,
}

pub fn generate_markdown(req: &SlideRequest, bullets: &[&str]) -> Result<String> {
    // MVP: ルールベースの雛形（後でLLM呼び出しに差し替え）
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", req.title));
    for (i, b) in bullets.iter().enumerate() {
        out.push_str(&format!("## {}. {}\n- Point A\n- Point B\n\n", i + 1, b));
    }
    Ok(out)
}
```

### slide-rs/cli/src/main.rs
```rust
use clap::CommandFactory;
use clap::Parser;
use clap_complete::Shell;
use clap_complete::generate;
use slide_arg0::arg0_dispatch_or_else;
use slide_chatgpt::apply_command::ApplyCommand;
use slide_chatgpt::apply_command::run_apply_command;
use slide_cli::LandlockCommand;
use slide_cli::SeatbeltCommand;
use slide_cli::login::run_login_status;
use slide_cli::login::run_login_with_api_key;
use slide_cli::login::run_login_with_chatgpt;
use slide_cli::login::run_logout;
use slide_cli::proto;
use slide_common::CliConfigOverrides;
use slide_exec::Cli as ExecCli;
use slide_tui::Cli as TuiCli;
use std::path::PathBuf;

use crate::proto::ProtoCli;

/// Slide CLI
///
/// If no subcommand is specified, options will be forwarded to the interactive CLI.
#[derive(Debug, Parser)]
#[clap(
    author,
    version,
    // If a sub‑command is given, ignore requirements of the default args.
    subcommand_negates_reqs = true,
    // The executable is sometimes invoked via a platform‑specific name like
    // `slide-x86_64-unknown-linux-musl`, but the help output should always use
    // the generic `slide` command name that users run.
    bin_name = "slide"
)]
struct MultitoolCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    interactive: TuiCli,

    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Run Slide non-interactively.
    #[clap(visible_alias = "e")]
    Exec(ExecCli),

    /// Manage login.
    Login(LoginCommand),

    /// Remove stored authentication credentials.
    Logout(LogoutCommand),

    /// Preview slide markdown
    #[clap(visible_alias = "p")]
    Preview(PreviewCommand),

    /// Run the Protocol stream via stdin/stdout
    Proto(ProtoCli),

    /// Generate shell completion scripts.
    Completion(CompletionCommand),

    /// Internal debugging commands.
    Debug(DebugArgs),

    /// Apply the latest diff produced by Slide agent as a `git apply` to your local working tree.
    #[clap(visible_alias = "a")]
    Apply(ApplyCommand),
}

#[derive(Debug, Parser)]
struct PreviewCommand {
    /// Path to markdown file to preview
    #[arg(value_name = "FILE")]
    file: PathBuf,
}

#[derive(Debug, Parser)]
struct CompletionCommand {
    /// Shell to generate completions for
    #[clap(value_enum, default_value_t = Shell::Bash)]
    shell: Shell,
}

#[derive(Debug, Parser)]
struct DebugArgs {
    #[command(subcommand)]
    cmd: DebugCommand,
}

#[derive(Debug, clap::Subcommand)]
enum DebugCommand {
    /// Run a command under Seatbelt (macOS only).
    Seatbelt(SeatbeltCommand),

    /// Run a command under Landlock+seccomp (Linux only).
    Landlock(LandlockCommand),
}

#[derive(Debug, Parser)]
struct LoginCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,

    #[arg(long = "api-key", value_name = "API_KEY")]
    api_key: Option<String>,

    #[command(subcommand)]
    action: Option<LoginSubcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum LoginSubcommand {
    /// Show login status.
    Status,
}

#[derive(Debug, Parser)]
struct LogoutCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,
}

fn main() -> anyhow::Result<()> {
    // Check if we're in Slide mode via environment variable
    let is_slide_mode = std::env::var("SLIDE_APP").is_ok();
    
    slide_arg0::arg0_dispatch_or_else(|slide_linux_sandbox_exe| async move {
        cli_main(slide_linux_sandbox_exe, is_slide_mode).await?;
        Ok(())
    })
}

async fn cli_main(slide_linux_sandbox_exe: Option<PathBuf>, is_slide_mode: bool) -> anyhow::Result<()> {
    let cli = MultitoolCli::parse();

    match cli.subcommand {
        None => {
            let mut tui_cli = cli.interactive;
            prepend_config_flags(&mut tui_cli.config_overrides, cli.config_overrides);
            let usage = slide_tui::run_main(tui_cli, slide_linux_sandbox_exe).await?;
            if !usage.is_zero() {
                println!("{}", slide_core::protocol::FinalOutput::from(usage));
            }
        }
        Some(Subcommand::Exec(mut exec_cli)) => {
            prepend_config_flags(&mut exec_cli.config_overrides, cli.config_overrides);
            slide_exec::run_main(exec_cli, slide_linux_sandbox_exe).await?;
        }
        Some(Subcommand::Login(mut login_cli)) => {
            prepend_config_flags(&mut login_cli.config_overrides, cli.config_overrides);
            match login_cli.action {
                Some(LoginSubcommand::Status) => {
                    run_login_status(login_cli.config_overrides).await;
                }
                None => {
                    if let Some(api_key) = login_cli.api_key {
                        run_login_with_api_key(login_cli.config_overrides, api_key).await;
                    } else {
                        run_login_with_chatgpt(login_cli.config_overrides).await;
                    }
                }
            }
        }
        Some(Subcommand::Logout(mut logout_cli)) => {
            prepend_config_flags(&mut logout_cli.config_overrides, cli.config_overrides);
            run_logout(logout_cli.config_overrides).await;
        }
        Some(Subcommand::Preview(preview_cli)) => {
            slide_tui::run_preview(preview_cli.file).await?;
        }
        Some(Subcommand::Proto(mut proto_cli)) => {
            prepend_config_flags(&mut proto_cli.config_overrides, cli.config_overrides);
            proto::run_main(proto_cli).await?;
        }
        Some(Subcommand::Completion(completion_cli)) => {
            print_completion(completion_cli);
        }
        Some(Subcommand::Debug(debug_args)) => match debug_args.cmd {
            DebugCommand::Seatbelt(mut seatbelt_cli) => {
                prepend_config_flags(&mut seatbelt_cli.config_overrides, cli.config_overrides);
                slide_cli::debug_sandbox::run_command_under_seatbelt(
                    seatbelt_cli,
                    slide_linux_sandbox_exe,
                )
                .await?;
            }
            DebugCommand::Landlock(mut landlock_cli) => {
                prepend_config_flags(&mut landlock_cli.config_overrides, cli.config_overrides);
                slide_cli::debug_sandbox::run_command_under_landlock(
                    landlock_cli,
                    slide_linux_sandbox_exe,
                )
                .await?;
            }
        },
        Some(Subcommand::Apply(mut apply_cli)) => {
            prepend_config_flags(&mut apply_cli.config_overrides, cli.config_overrides);
            run_apply_command(apply_cli, None).await?;
        }
    }

    Ok(())
}

/// Prepend root-level overrides so they have lower precedence than
/// CLI-specific ones specified after the subcommand (if any).
fn prepend_config_flags(
    subcommand_config_overrides: &mut CliConfigOverrides,
    cli_config_overrides: CliConfigOverrides,
) {
    subcommand_config_overrides
        .raw_overrides
        .splice(0..0, cli_config_overrides.raw_overrides);
}

fn print_completion(cmd: CompletionCommand) {
    let mut app = MultitoolCli::command();
    let name = "slide";
    generate(cmd.shell, &mut app, name, &mut std::io::stdout());
}
```

### slide-rs/Cargo.toml
```toml
[workspace]
members = [
    "ansi-escape",
    "apply-patch",
    "arg0",
    "cli",
    "common",
    "core",
    "exec",
    "execpolicy",
    "file-search",
    "linux-sandbox",
    "protocol",
    "tui",
]
resolver = "2"

[workspace.package]
version = "0.0.1"
edition = "2024"

[workspace.lints]
rust = {}

[workspace.lints.clippy]
expect_used = "deny"
uninlined_format_args = "deny"
unwrap_used = "deny"

[profile.release]
lto = "fat"
strip = "symbols"
codegen-units = 1

[patch.crates-io]
ratatui = { git = "https://github.com/nornagon/ratatui", branch = "nornagon-v0.29.0-patch" }
```

### slide-rs/Cargo.lock
```toml
# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 3

[[package]]
name = "slide"
version = "0.0.1"
```

### slide-rs/clippy.toml
```toml
allow-expect-in-tests = true
allow-unwrap-in-tests = true
disallowed-methods = [
    { path = "ratatui::style::Color::Rgb", reason = "Use ANSI colors, which work better in various terminal themes." },
    { path = "ratatui::style::Color::Indexed", reason = "Use ANSI colors, which work better in various terminal themes." },
    { path = "ratatui::style::Stylize::white", reason = "Avoid hardcoding white; prefer default fg or dim/bold. Exception: Disable this rule if rendering over a hardcoded ANSI background." },
    { path = "ratatui::style::Stylize::black", reason = "Avoid hardcoding black; prefer default fg or dim/bold. Exception: Disable this rule if rendering over a hardcoded ANSI background." },
    { path = "ratatui::style::Stylize::yellow", reason = "Avoid yellow; prefer other colors in `tui/styles.md`." },
]
```

### slide-rs/rust-toolchain.toml
```toml
[toolchain]
channel = "1.89.0"
components = [ "clippy", "rustfmt", "rust-src"]
```

### slide-rs/rustfmt.toml
```toml
edition = "2024"
imports_granularity = "Item"
```

### slide-rs/justfile
```makefile
set positional-arguments

# Display help
help:
    just -l

# `slide`
slide *args:
    cargo run --bin slide -- "$@"

# `slide exec`
exec *args:
    cargo run --bin slide -- exec "$@"

# `slide tui`
tui *args:
    cargo run --bin slide -- tui "$@"

# `slide preview`
preview *args:
    cargo run --bin slide -- preview "$@"

# Run the CLI version of the file-search crate.
file-search *args:
    cargo run --bin slide-file-search -- "$@"

# format code
fmt:
    cargo fmt -- --config imports_granularity=Item

fix *args:
    cargo clippy --fix --all-features --tests --allow-dirty "$@"

install:
    rustup show active-toolchain
    cargo fetch
```

### slide-rs/docs/protocol_v1.md
```markdown
Overview of Protocol Defined in protocol.rs and agent.rs.

The goal of this document is to define terminology used in the Slide system and explain the expected behavior of the system.

## Entities

These are entities exist on the slide backend. The intent of this section is to establish vocabulary and construct a shared mental model for the `Slide` core system.

0. `Model`
   - In our case, this is the Responses REST API
1. `Slide`
   - The core engine of slide
   - Runs locally, either in a background thread or separate process
   - Communicated to via a queue pair – SQ (Submission Queue) / EQ (Event Queue)
   - Takes user input, makes requests to the `Model`, executes commands and applies patches.
2. `Session`
   - The `Slide`'s current configuration and state
   - `Slide` starts with no `Session`, and it is initialized by `Op::ConfigureSession`, which should be the first message sent by the UI.
   - The current `Session` can be reconfigured with additional `Op::ConfigureSession` calls.
   - Any running execution is aborted when the session is reconfigured.
3. `Task`
   - A `Task` is `Slide` executing work in response to user input.
   - `Session` has at most one `Task` running at a time.
   - Receiving `Op::UserInput` starts a `Task`
   - Consists of a series of `Turn`s
   - The `Task` executes to until:
     - The `Model` completes the task and there is no output to feed into an additional `Turn`
     - Additional `Op::UserInput` aborts the current task and starts a new one
     - UI interrupts with `Op::Interrupt`
     - Fatal errors are encountered, eg. `Model` connection exceeding retry limits
     - Blocked by user approval (executing a command or patch)
4. `Turn`
   - One cycle of iteration in a `Task`, consists of:
     - A request to the `Model` - (initially) prompt + (optional) `last_response_id`, or (in loop) previous turn output
     - The `Model` streams responses back in an SSE, which are collected until "completed" message and the SSE terminates
     - `Slide` then executes command(s), applies patch(es), and outputs message(s) returned by the `Model`
     - Pauses to request approval when necessary
   - The output of one `Turn` is the input to the next `Turn`
   - A `Turn` yielding no output terminates the `Task`

The term "UI" is used to refer to the application driving `Slide`. This may be the CLI / TUI chat-like interface that users operate, or it may be a GUI interface like a VSCode extension. The UI is external to `Slide`, as `Slide` is intended to be operated by arbitrary UI implementations.

When a `Turn` completes, the `response_id` from the `Model`'s final `response.completed` message is stored in the `Session` state to resume the thread given the next `Op::UserInput`. The `response_id` is also returned in the `EventMsg::TurnComplete` to the UI, which can be used to fork the thread from an earlier point by providing it in the `Op::UserInput`.

Since only 1 `Task` can be run at a time, for parallel tasks it is recommended that a single `Slide` be run for each thread of work.

## Interface

- `Slide`
  - Communicates with UI via a `SQ` (Submission Queue) and `EQ` (Event Queue).
- `Submission`
  - These are messages sent on the `SQ` (UI -> `Slide`)
  - Has an string ID provided by the UI, referred to as `sub_id`
  - `Op` refers to the enum of all possible `Submission` payloads
    - This enum is `non_exhaustive`; variants can be added at future dates
- `Event`
  - These are messages sent on the `EQ` (`Slide` -> UI)
  - Each `Event` has a non-unique ID, matching the `sub_id` from the `Op::UserInput` that started the current task.
  - `EventMsg` refers to the enum of all possible `Event` payloads
    - This enum is `non_exhaustive`; variants can be added at future dates
    - It should be expected that new `EventMsg` variants will be added over time to expose more detailed information about the model's actions.

For complete documentation of the `Op` and `EventMsg` variants, refer to protocol.rs. Some example payload types:

- `Op`
  - `Op::UserInput` – Any input from the user to kick off a `Task`
  - `Op::Interrupt` – Interrupts a running task
  - `Op::ExecApproval` – Approve or deny code execution
- `EventMsg`
  - `EventMsg::AgentMessage` – Messages from the `Model`
  - `EventMsg::ExecApprovalRequest` – Request approval from user to execute a command
  - `EventMsg::TaskComplete` – A task completed successfully
  - `EventMsg::Error` – A task stopped with an error
  - `EventMsg::TurnComplete` – Contains a `response_id` bookmark for last `response_id` executed by the task. This can be used to continue the task at a later point in time, perhaps with additional user input.

The `response_id` returned from each task matches the OpenAI `response_id` stored in the API's `/responses` endpoint. It can be stored and used in future `Sessions` to resume threads of work.

## Transport

Can operate over any transport that supports bi-directional streaming. - cross-thread channels - IPC channels - stdin/stdout - TCP - HTTP2 - gRPC

Non-framed transports, such as stdin/stdout and TCP, should use newline-delimited JSON in sending messages.
```

### slide-rs/scripts/create_github_release.sh
```bash
#!/bin/bash

set -euo pipefail

# By default, this script uses a version based on the current date and time.
# If you want to specify a version, pass it as the first argument. Example:
#
#     ./scripts/create_github_release.sh 0.1.0-alpha.4
#
# The value will be used to update the `version` field in `Cargo.toml`.

# Change to the root of the Cargo workspace.
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# Cancel if there are uncommitted changes.
if ! git diff --quiet || ! git diff --cached --quiet || [ -n "$(git ls-files --others --exclude-standard)" ]; then
  echo "ERROR: You have uncommitted or untracked changes." >&2
  exit 1
fi

# Fail if in a detached HEAD state.
CURRENT_BRANCH=$(git symbolic-ref --short -q HEAD 2>/dev/null || true)
if [ -z "${CURRENT_BRANCH:-}" ]; then
  echo "ERROR: Could not determine the current branch (detached HEAD?)." >&2
  echo "       Please run this script from a checked-out branch." >&2
  exit 1
fi

# Ensure we are on the 'main' branch before proceeding.
if [ "${CURRENT_BRANCH}" != "main" ]; then
  echo "ERROR: Releases must be created from the 'main' branch (current: '${CURRENT_BRANCH}')." >&2
  echo "       Please switch to 'main' and try again." >&2
  exit 1
fi

# Ensure the current local commit on 'main' is present on 'origin/main'.
# This guarantees we only create releases from commits that are already on
# the canonical repository (https://github.com/yourorg/slide).
if ! git fetch --quiet origin main; then
  echo "ERROR: Failed to fetch 'origin/main'. Ensure the 'origin' remote is configured and reachable." >&2
  exit 1
fi

if ! git merge-base --is-ancestor HEAD origin/main; then
  echo "ERROR: Your local 'main' HEAD commit is not present on 'origin/main'." >&2
  echo "       Please push your commits first (git push origin main) or check out a commit on 'origin/main'." >&2
  exit 1
fi

# Create a new branch for the release and make a commit with the new version.
if [ $# -ge 1 ]; then
  VERSION="$1"
else
  VERSION=$(printf '0.0.%d' "$(date +%y%m%d%H%M)")
fi
TAG="rust-v$VERSION"
git checkout -b "$TAG"
perl -i -pe "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
git add Cargo.toml
git commit -m "Release $VERSION"
git tag -a "$TAG" -m "Release $VERSION"
git push origin "refs/tags/$TAG"

git checkout "$CURRENT_BRANCH"
```

### slide-rs/ansi-escape/Cargo.toml
```toml
[package]
edition = "2024"
name = "slide-ansi-escape"
version = { workspace = true }

[lib]
name = "slide_ansi_escape"
path = "src/lib.rs"

[dependencies]
ansi-to-tui = "7.0.0"
ratatui = { version = "0.29.0", features = [
    "unstable-rendered-line-info",
    "unstable-widget-ref",
] }
tracing = { version = "0.1.41", features = ["log"] }
```

### slide-rs/ansi-escape/src/lib.rs
```rust
use ansi_to_tui::Error;
use ansi_to_tui::IntoText;
use ratatui::text::Line;
use ratatui::text::Text;

/// This function should be used when the contents of `s` are expected to match
/// a single line. If multiple lines are found, a warning is logged and only the
/// first line is returned.
pub fn ansi_escape_line(s: &str) -> Line<'static> {
    let text = ansi_escape(s);
    match text.lines.as_slice() {
        [] => Line::from(""),
        [only] => only.clone(),
        [first, rest @ ..] => {
            tracing::warn!("ansi_escape_line: expected a single line, got {first:?} and {rest:?}");
            first.clone()
        }
    }
}

pub fn ansi_escape(s: &str) -> Text<'static> {
    // to_text() claims to be faster, but introduces complex lifetime issues
    // such that it's not worth it.
    match s.into_text() {
        Ok(text) => text,
        Err(err) => match err {
            Error::NomError(message) => {
                tracing::error!(
                    "ansi_to_tui NomError docs claim should never happen when parsing `{s}`: {message}"
                );
                panic!();
            }
            Error::Utf8Error(utf8error) => {
                tracing::error!("Utf8Error: {utf8error}");
                panic!();
            }
        },
    }
}
```

### slide-rs/apply-patch/Cargo.toml
```toml
[package]
edition = "2024"
name = "slide-apply-patch"
version = { workspace = true }

[lib]
name = "slide_apply_patch"
path = "src/lib.rs"

[[bin]]
name = "apply_patch"
path = "src/main.rs"

[lints]
workspace = true

[dependencies]
anyhow = "1"
similar = "2.7.0"
thiserror = "2.0.12"
tree-sitter = "0.25.8"
tree-sitter-bash = "0.25.0"

[dev-dependencies]
assert_cmd = "2"
pretty_assertions = "1.4.1"
tempfile = "3.13.0"
```

### slide-rs/apply-patch/src/lib.rs
```rust
mod parser;
mod seek_sequence;
mod standalone_executable;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::str::Utf8Error;

use anyhow::Context;
use anyhow::Result;
pub use parser::Hunk;
pub use parser::ParseError;
use parser::ParseError::*;
use parser::UpdateFileChunk;
pub use parser::parse_patch;
use similar::TextDiff;
use thiserror::Error;
use tree_sitter::LanguageError;
use tree_sitter::Parser;
use tree_sitter_bash::LANGUAGE as BASH;

pub use standalone_executable::main;

/// Detailed instructions for slide-agent on how to use the `apply_patch` tool.
pub const APPLY_PATCH_TOOL_INSTRUCTIONS: &str = include_str!("../apply_patch_tool_instructions.md");

const APPLY_PATCH_COMMANDS: [&str; 2] = ["apply_patch", "applypatch"];

#[derive(Debug, Error, PartialEq)]
pub enum ApplyPatchError {
    #[error(transparent)]
    ParseError(#[from] ParseError),
    #[error(transparent)]
    IoError(#[from] IoError),
    /// Error that occurs while computing replacements when applying patch chunks
    #[error("{0}")]
    ComputeReplacements(String),
}

#[derive(Debug, Error, PartialEq)]
pub struct IoError {
    pub context: String,
    #[source]
    pub source: std::io::Error,
}

impl From<std::io::Error> for ApplyPatchError {
    fn from(err: std::io::Error) -> Self {
        ApplyPatchError::IoError(IoError {
            context: "I/O error".to_string(),
            source: err,
        })
    }
}

pub fn apply_patch_to_files(patch_content: &str, dry_run: bool) -> Result<Vec<PathBuf>, ApplyPatchError> {
    let hunks = parse_patch(patch_content)?;
    let mut modified_files = Vec::new();

    for hunk in hunks {
        match hunk {
            Hunk::Add { path, lines } => {
                modified_files.push(path.clone());
                if !dry_run {
                    std::fs::write(&path, lines.join("\n"))?;
                }
            }
            Hunk::Delete { path } => {
                modified_files.push(path.clone());
                if !dry_run {
                    std::fs::remove_file(&path)?;
                }
            }
            Hunk::Update { path, changes } => {
                modified_files.push(path.clone());
                if !dry_run {
                    apply_update_hunk(&path, &changes)?;
                }
            }
        }
    }

    Ok(modified_files)
}

fn apply_update_hunk(path: &Path, changes: &[UpdateFileChunk]) -> Result<(), ApplyPatchError> {
    let content = std::fs::read_to_string(path)?;
    let mut result = content;

    for change in changes {
        match change {
            UpdateFileChunk::Context(_) => {
                // Context lines are just for reference, no action needed
            }
            UpdateFileChunk::Change { removals, additions } => {
                // Apply the change using text diff
                let diff = TextDiff::from_lines(&removals.join("\n"), &additions.join("\n"));
                // This is a simplified implementation
                result = additions.join("\n");
            }
        }
    }

    std::fs::write(path, result)?;
    Ok(())
}
```

### slide-rs/apply-patch/src/main.rs
```rust
pub fn main() -> ! {
    slide_apply_patch::main()
}
```

### slide-rs/apply-patch/src/parser.rs
```rust
//! This module is responsible for parsing & validating a patch into a list of "hunks".
//! (It does not attempt to actually check that the patch can be applied to the filesystem.)
//!
//! The official Lark grammar for the apply-patch format is:
//!
//! start: begin_patch hunk+ end_patch
//! begin_patch: "*** Begin Patch" LF
//! end_patch: "*** End Patch" LF?
//!
//! hunk: add_hunk | delete_hunk | update_hunk
//! add_hunk: "*** Add File: " filename LF add_line+
//! delete_hunk: "*** Delete File: " filename LF
//! update_hunk: "*** Update File: " filename LF change_move? change?
//! filename: /(.+)/
//! add_line: "+" /(.+)/ LF -> line
//!
//! change_move: "*** Move to: " filename LF
//! change: (change_context | change_line)+ eof_line?
//! change_context: ("@@" | "@@ " /(.+)/) LF
//! change_line: ("+" | "-" | " ") /(.+)/ LF
//! eof_line: "*** End of File" LF
//!
//! The parser below is a little more lenient than the explicit spec and allows for
//! leading/trailing whitespace around patch markers.
use std::path::PathBuf;
use thiserror::Error;

const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
const END_PATCH_MARKER: &str = "*** End Patch";
const ADD_FILE_MARKER: &str = "*** Add File: ";
const DELETE_FILE_MARKER: &str = "*** Delete File: ";
const UPDATE_FILE_MARKER: &str = "*** Update File: ";
const MOVE_TO_MARKER: &str = "*** Move to: ";
const EOF_MARKER: &str = "*** End of File";
const CHANGE_CONTEXT_MARKER: &str = "@@ ";
const EMPTY_CHANGE_CONTEXT_MARKER: &str = "@@";

#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("Expected begin patch marker")]
    ExpectedBeginPatchMarker,
    #[error("Expected end patch marker")]
    ExpectedEndPatchMarker,
    #[error("Invalid file operation: {0}")]
    InvalidFileOperation(String),
    #[error("Parse error at line {line}: {message}")]
    ParseErrorAtLine { line: usize, message: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Hunk {
    Add { path: PathBuf, lines: Vec<String> },
    Delete { path: PathBuf },
    Update { path: PathBuf, changes: Vec<UpdateFileChunk> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateFileChunk {
    Context(String),
    Change { removals: Vec<String>, additions: Vec<String> },
}

pub fn parse_patch(content: &str) -> Result<Vec<Hunk>, ParseError> {
    let lines: Vec<&str> = content.lines().collect();
    
    if lines.is_empty() || !lines[0].trim().starts_with(BEGIN_PATCH_MARKER) {
        return Err(ParseError::ExpectedBeginPatchMarker);
    }

    let mut hunks = Vec::new();
    let mut i = 1;

    while i < lines.len() {
        if lines[i].trim().starts_with(END_PATCH_MARKER) {
            break;
        }

        if lines[i].trim().starts_with(ADD_FILE_MARKER) {
            let path = PathBuf::from(lines[i].trim_start_matches(ADD_FILE_MARKER).trim());
            i += 1;
            let mut file_lines = Vec::new();
            
            while i < lines.len() && lines[i].starts_with('+') {
                file_lines.push(lines[i][1..].to_string());
                i += 1;
            }
            
            hunks.push(Hunk::Add { path, lines: file_lines });
        } else if lines[i].trim().starts_with(DELETE_FILE_MARKER) {
            let path = PathBuf::from(lines[i].trim_start_matches(DELETE_FILE_MARKER).trim());
            hunks.push(Hunk::Delete { path });
            i += 1;
        } else if lines[i].trim().starts_with(UPDATE_FILE_MARKER) {
            let path = PathBuf::from(lines[i].trim_start_matches(UPDATE_FILE_MARKER).trim());
            i += 1;
            
            let changes = Vec::new(); // Simplified for MVP
            hunks.push(Hunk::Update { path, changes });
        } else {
            i += 1;
        }
    }

    Ok(hunks)
}
```

### slide-rs/apply-patch/src/seek_sequence.rs
```rust
/// Attempt to find the sequence of `pattern` lines within `lines` beginning at or after `start`.
/// Returns the starting index of the match or `None` if not found. Matches are attempted with
/// decreasing strictness: exact match, then ignoring trailing whitespace, then ignoring leading
/// and trailing whitespace. When `eof` is true, we first try starting at the end-of-file (so that
/// patterns intended to match file endings are applied at the end), and fall back to searching
/// from `start` if needed.
///
/// Special cases handled defensively:
///  • Empty `pattern` → returns `Some(start)` (no-op match)
///  • `pattern.len() > lines.len()` → returns `None` (cannot match, avoids
///    out‑of‑bounds panic that occurred pre‑2025‑04‑12)
pub(crate) fn seek_sequence(
    lines: &[String],
    pattern: &[String],
    start: usize,
    eof: bool,
) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }

    // When the pattern is longer than the available input there is no possible
    // match. Early‑return to avoid the out‑of‑bounds slice that would occur in
    // the search loops below (previously caused a panic when
    // `pattern.len() > lines.len()`).
    if pattern.len() > lines.len() {
        return None;
    }
    let search_start = if eof && lines.len() >= pattern.len() {
        lines.len() - pattern.len()
    } else {
        start.min(lines.len())
    };

    // Try exact match first
    for i in search_start..=(lines.len().saturating_sub(pattern.len())) {
        let slice = &lines[i..i + pattern.len()];
        if slice == pattern {
            return Some(i);
        }
    }

    // Try ignoring trailing whitespace
    for i in search_start..=(lines.len().saturating_sub(pattern.len())) {
        let slice = &lines[i..i + pattern.len()];
        if slice.iter().zip(pattern.iter()).all(|(a, b)| a.trim_end() == b.trim_end()) {
            return Some(i);
        }
    }

    // Try ignoring leading and trailing whitespace
    for i in search_start..=(lines.len().saturating_sub(pattern.len())) {
        let slice = &lines[i..i + pattern.len()];
        if slice.iter().zip(pattern.iter()).all(|(a, b)| a.trim() == b.trim()) {
            return Some(i);
        }
    }

    None
}
```

### slide-rs/apply-patch/src/standalone_executable.rs
```rust
use std::io::Read;
use std::io::Write;

pub fn main() -> ! {
    let exit_code = run_main();
    std::process::exit(exit_code);
}

/// We would prefer to return `std::process::ExitCode`, but its `exit_process()`
/// method is still a nightly API and we want main() to return !.
pub fn run_main() -> i32 {
    // Expect either one argument (the full apply_patch payload) or read it from stdin.
    let mut args = std::env::args_os();
    let _argv0 = args.next();

    let patch_arg = match args.next() {
        Some(arg) => match arg.into_string() {
            Ok(s) => s,
            Err(_) => {
                eprintln!("Error: apply_patch requires a UTF-8 PATCH argument.");
                return 1;
            }
        },
        None => {
            // No argument provided; attempt to read the patch from stdin.
            let mut buf = String::new();
            match std::io::stdin().read_to_string(&mut buf) {
                Ok(_) => {
                    if buf.is_empty() {
                        eprintln!("Usage: apply_patch 'PATCH'\n       echo 'PATCH' | apply-patch");
                        return 1;
                    }
                    buf
                }
                Err(e) => {
                    eprintln!("Error reading from stdin: {e}");
                    return 1;
                }
            }
        }
    };

    if let Err(e) = crate::apply_patch_to_files(&patch_arg, false) {
        eprintln!("Error applying patch: {e}");
        return 1;
    }

    0
}
```

### slide-rs/apply-patch/tests/all.rs
```rust
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use slide_apply_patch::{parse_patch, Hunk, apply_patch_to_files};

#[test]
fn test_parse_simple_patch() {
    let patch_content = r#"
*** Begin Patch
*** Add File: hello.txt
+Hello, world!
+This is a test file.
*** End Patch
"#;

    let hunks = parse_patch(patch_content).unwrap();
    assert_eq!(hunks.len(), 1);
    
    match &hunks[0] {
        Hunk::Add { path, lines } => {
            assert_eq!(path, &PathBuf::from("hello.txt"));
            assert_eq!(lines.len(), 2);
            assert_eq!(lines[0], "Hello, world!");
            assert_eq!(lines[1], "This is a test file.");
        }
        _ => panic!("Expected Add hunk"),
    }
}

#[test]
fn test_apply_patch() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    let patch_content = format!(r#"
*** Begin Patch
*** Add File: {}
+Line 1
+Line 2
*** End Patch
"#, file_path.to_string_lossy());

    // Change to temp directory for relative path resolution
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let modified_files = apply_patch_to_files(&patch_content, false).unwrap();
    assert_eq!(modified_files.len(), 1);

    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "Line 1\nLine 2");
}
```

### slide-rs/arg0/Cargo.toml
```toml
[package]
edition = "2024"
name = "slide-arg0"
version = { workspace = true }

[lib]
name = "slide_arg0"
path = "src/lib.rs"

[lints]
workspace = true

[dependencies]
anyhow = "1"
slide-apply-patch = { path = "../apply-patch" }
slide-core = { path = "../core" }
slide-linux-sandbox = { path = "../linux-sandbox" }
dotenvy = "0.15.7"
tempfile = "3"
tokio = { version = "1", features = ["rt-multi-thread"] }
```

### slide-rs/arg0/src/lib.rs
```rust
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;

use slide_core::SLIDE_APPLY_PATCH_ARG1;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::TempDir;

const LINUX_SANDBOX_ARG0: &str = "slide-linux-sandbox";
const APPLY_PATCH_ARG0: &str = "apply_patch";
const MISSPELLED_APPLY_PATCH_ARG0: &str = "applypatch";

/// While we want to deploy the Slide CLI as a single executable for simplicity,
/// we also want to expose some of its functionality as distinct CLIs, so we use
/// the "arg0 trick" to determine which CLI to dispatch. This effectively allows
/// us to simulate deploying multiple executables as a single binary on Mac and
/// Linux (but not Windows).
///
/// When the current executable is invoked through the hard-link or alias named
/// `slide-linux-sandbox` we *directly* execute
/// [`slide_linux_sandbox::run_main`] (which never returns). Otherwise we:
///
/// 1.  Use [`dotenvy::from_path`] and [`dotenvy::dotenv`] to modify the
///     environment before creating any threads.
/// 2.  Construct a Tokio multi-thread runtime.
/// 3.  Derive the path to the current executable (so children can re-invoke the
///     sandbox) when running on Linux.
/// 4.  Execute the provided async `main_fn` inside that runtime, forwarding any
///     error. Note that `main_fn` receives `slide_linux_sandbox_exe:
///     Option<PathBuf>`.
///
/// This function never returns.
pub fn arg0_dispatch_or_else<F, Fut>(main_fn: F) -> !
where
    F: FnOnce(Option<PathBuf>) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let arg0 = match std::env::args().next() {
        Some(arg) => arg,
        None => {
            eprintln!("Error: No arg0 found");
            std::process::exit(1);
        }
    };

    let basename = Path::new(&arg0)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    // Check for specific arg0 patterns
    match basename {
        LINUX_SANDBOX_ARG0 => {
            #[cfg(target_os = "linux")]
            slide_linux_sandbox::run_main();
            #[cfg(not(target_os = "linux"))]
            {
                eprintln!("Linux sandbox is only available on Linux");
                std::process::exit(1);
            }
        }
        APPLY_PATCH_ARG0 | MISSPELLED_APPLY_PATCH_ARG0 => {
            slide_apply_patch::main();
        }
        _ => {
            // Default slide CLI behavior
            setup_environment_and_run(main_fn);
        }
    }
}

fn setup_environment_and_run<F, Fut>(main_fn: F) -> !
where
    F: FnOnce(Option<PathBuf>) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    // Load environment variables from .env files
    dotenvy::from_path(".env.local").ok();
    dotenvy::dotenv().ok();

    // Create Tokio runtime
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Failed to create Tokio runtime: {e}");
            std::process::exit(1);
        }
    };

    // Determine sandbox executable path for Linux
    let slide_linux_sandbox_exe = get_linux_sandbox_exe();

    // Run the main function
    let result = runtime.block_on(main_fn(slide_linux_sandbox_exe));
    
    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn get_linux_sandbox_exe() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::env::current_exe().ok()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
```

### slide-rs/chatgpt/Cargo.toml
```toml
[package]
edition = "2024"
name = "slide-chatgpt"
version = { workspace = true }

[lints]
workspace = true

[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
slide-common = { path = "../common", features = ["cli"] }
slide-core = { path = "../core" }
slide-login = { path = "../login" }
reqwest = { version = "0.12", features = ["json", "stream"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
tempfile = "3"
```

### slide-rs/chatgpt/src/lib.rs
```rust
pub mod apply_command;
mod chatgpt_client;
mod chatgpt_token;
pub mod get_task;
```

### slide-rs/chatgpt/src/apply_command.rs
```rust
use std::path::PathBuf;

use clap::Parser;
use slide_common::CliConfigOverrides;
use slide_core::config::Config;
use slide_core::config::ConfigOverrides;

use crate::chatgpt_token::init_chatgpt_token_from_auth;
use crate::get_task::GetTaskResponse;
use crate::get_task::OutputItem;
use crate::get_task::PrOutputItem;
use crate::get_task::get_task;

/// Applies the latest diff from a Slide agent task.
#[derive(Debug, Parser)]
pub struct ApplyCommand {
    pub task_id: String,

    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,
}

pub async fn run_apply_command(
    apply_cli: ApplyCommand,
    cwd: Option<PathBuf>,
) -> anyhow::Result<()> {
    let config = Config::load_with_cli_overrides(
        apply_cli
            .config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?,
    )?;

    let get_task_response = get_task(&config, &apply_cli.task_id).await?;

    match get_task_response {
        GetTaskResponse::Found { result } => {
            if let Some(result) = result {
                if let Some(last_output) = result.iter().rev().next() {
                    match last_output {
                        OutputItem::PrOutput(pr_output) => {
                            println!("Applying patch from task {}", apply_cli.task_id);
                            apply_pr_output(pr_output, cwd).await?;
                        }
                        _ => {
                            return Err(anyhow::anyhow!(
                                "No diff found in task {}",
                                apply_cli.task_id
                            ));
                        }
                    }
                }
            }
        }
        GetTaskResponse::NotFound => {
            return Err(anyhow::anyhow!("Task {} not found", apply_cli.task_id));
        }
    }

    Ok(())
}

async fn apply_pr_output(pr_output: &PrOutputItem, _cwd: Option<PathBuf>) -> anyhow::Result<()> {
    if let Some(diff) = &pr_output.diff {
        slide_apply_patch::apply_patch_to_files(diff, false)?;
        println!("Patch applied successfully");
    } else {
        return Err(anyhow::anyhow!("No diff found in PR output"));
    }
    Ok(())
}
```

### slide-rs/chatgpt/src/chatgpt_client.rs
```rust
use slide_core::config::Config;
use slide_core::user_agent::get_slide_user_agent;

use crate::chatgpt_token::get_chatgpt_token_data;
use crate::chatgpt_token::init_chatgpt_token_from_auth;

use anyhow::Context;
use serde::de::DeserializeOwned;

/// Make a GET request to the ChatGPT backend API.
pub(crate) async fn chatgpt_get_request<T: DeserializeOwned>(
    config: &Config,
    path: String,
) -> anyhow::Result<T> {
    let chatgpt_base_url = &config.chatgpt_base_url;
    init_chatgpt_token_from_auth(&config.slide_home).await?;

    // Make direct HTTP request to ChatGPT backend API with the token
    let client = reqwest::Client::new();
    let url = format!("{chatgpt_base_url}{path}");

    let token =
        get_chatgpt_token_data().ok_or_else(|| anyhow::anyhow!("ChatGPT token not available"))?;

    let account_id = token.account_id.ok_or_else(|| {
        anyhow::anyhow!("ChatGPT account ID not available, please re-run `slide login`")
    })?;

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token.access_token))
        .header("User-Agent", get_slide_user_agent())
        .header("Content-Type", "application/json")
        .send()
        .await
        .context("Failed to send request to ChatGPT API")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "ChatGPT API request failed with status {}: {}",
            status,
            error_text
        ));
    }

    let result: T = response
        .json()
        .await
        .context("Failed to parse JSON response")?;

    Ok(result)
}
```

### slide-rs/chatgpt/src/chatgpt_token.rs
```rust
use slide_login::AuthMode;
use slide_login::SlideAuth;
use std::path::Path;
use std::sync::LazyLock;
use std::sync::RwLock;

use slide_login::TokenData;

static CHATGPT_TOKEN: LazyLock<RwLock<Option<TokenData>>> = LazyLock::new(|| RwLock::new(None));

pub fn get_chatgpt_token_data() -> Option<TokenData> {
    CHATGPT_TOKEN.read().ok()?.clone()
}

pub fn set_chatgpt_token_data(value: TokenData) {
    if let Ok(mut guard) = CHATGPT_TOKEN.write() {
        *guard = Some(value);
    }
}

/// Initialize the ChatGPT token from auth.json file
pub async fn init_chatgpt_token_from_auth(slide_home: &Path) -> std::io::Result<()> {
    let auth = SlideAuth::from_slide_home(slide_home, AuthMode::ChatGPT)?;
    if let Some(auth) = auth {
        let token_data = auth.get_token_data().await?;
        set_chatgpt_token_data(token_data);
    }
    Ok(())
}
```

### slide-rs/chatgpt/src/get_task.rs
```rust
use slide_core::config::Config;
use serde::Deserialize;

use crate::chatgpt_client::chatgpt_get_request;

#[derive(Debug, Deserialize)]
pub struct GetTaskResponse {
    pub current_diff_task_turn: Option<AssistantTurn>,
}

// Only relevant fields for our extraction
#[derive(Debug, Deserialize)]
pub struct AssistantTurn {
    pub output_items: Vec<OutputItem>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum OutputItem {
    #[serde(rename = "pr")]
    Pr(PrOutputItem),

    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct PrOutputItem {
    pub output_diff: OutputDiff,
}

#[derive(Debug, Deserialize)]
pub struct OutputDiff {
    pub diff: String,
}

pub(crate) async fn get_task(config: &Config, task_id: String) -> anyhow::Result<GetTaskResponse> {
    let path = format!("/wham/tasks/{task_id}");
    chatgpt_get_request(config, path).await
}
```

### slide-rs/chatgpt/tests/all.rs
```rust
// Single integration test binary that aggregates all test modules.
// The submodules live in `tests/suite/`.
mod suite;
```

### slide-rs/cli/src/debug_sandbox.rs
```rust
use std::path::PathBuf;

use slide_common::CliConfigOverrides;
use slide_core::config::Config;
use slide_core::config::ConfigOverrides;
use slide_core::exec_env::create_env;
use slide_core::landlock::spawn_command_under_linux_sandbox;
use slide_core::seatbelt::spawn_command_under_seatbelt;
use slide_core::spawn::StdioPolicy;
use slide_protocol::config_types::SandboxMode;

use crate::LandlockCommand;
use crate::SeatbeltCommand;
use crate::exit_status::handle_exit_status;

pub async fn run_command_under_seatbelt(
    command: SeatbeltCommand,
    slide_linux_sandbox_exe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let SeatbeltCommand {
        full_auto,
        config_overrides,
        command,
    } = command;
    run_command_under_sandbox(
        full_auto,
        command,
        config_overrides,
        slide_linux_sandbox_exe,
        SandboxType::Seatbelt,
    )
    .await
}

pub async fn run_command_under_landlock(
    command: LandlockCommand,
    slide_linux_sandbox_exe: Option<PathBuf>,
) -> anyhow::Result<()> {
    let LandlockCommand {
        full_auto,
        config_overrides,
        command,
    } = command;
    run_command_under_sandbox(
        full_auto,
        command,
        config_overrides,
        slide_linux_sandbox_exe,
        SandboxType::Landlock,
    )
    .await
}

enum SandboxType {
    Seatbelt,
    Landlock,
}

async fn run_command_under_sandbox(
    full_auto: bool,
    command: Vec<String>,
    config_overrides: CliConfigOverrides,
    slide_linux_sandbox_exe: Option<PathBuf>,
    sandbox_type: SandboxType,
) -> anyhow::Result<()> {
    let sandbox_mode = create_sandbox_mode(full_auto);
    let cwd = std::env::current_dir()?;
    let config = Config::load_with_cli_overrides(
        config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?,
        ConfigOverrides {
            sandbox_mode: Some(sandbox_mode),
            slide_linux_sandbox_exe,
            ..Default::default()
        },
    )?;
    let stdio_policy = StdioPolicy::Inherit;
    let env = create_env(&config.shell_environment_policy);

    let mut child = match sandbox_type {
        SandboxType::Seatbelt => {
            spawn_command_under_seatbelt(command, &config.sandbox_policy, cwd, stdio_policy, env)
                .await?
        }
        SandboxType::Landlock => {
            #[expect(clippy::expect_used)]
            let slide_linux_sandbox_exe = config
                .slide_linux_sandbox_exe
                .expect("slide-linux-sandbox executable not found");
            spawn_command_under_linux_sandbox(
                slide_linux_sandbox_exe,
                command,
                &config.sandbox_policy,
                cwd,
                stdio_policy,
                env,
            )
            .await?
        }
    };
    let status = child.wait().await?;

    handle_exit_status(status);
}

pub fn create_sandbox_mode(full_auto: bool) -> SandboxMode {
    if full_auto {
        SandboxMode::WorkspaceWrite
    } else {
        SandboxMode::ReadOnly
    }
}
```

### slide-rs/cli/src/exit_status.rs
```rust
#[cfg(unix)]
pub(crate) fn handle_exit_status(status: std::process::ExitStatus) -> ! {
    use std::os::unix::process::ExitStatusExt;

    // Use ExitStatus to derive the exit code.
    if let Some(code) = status.code() {
        std::process::exit(code);
    } else if let Some(signal) = status.signal() {
        std::process::exit(128 + signal);
    } else {
        std::process::exit(1);
    }
}

#[cfg(windows)]
pub(crate) fn handle_exit_status(status: std::process::ExitStatus) -> ! {
    if let Some(code) = status.code() {
        std::process::exit(code);
    } else {
        // Rare on Windows, but if it happens: use fallback code.
        std::process::exit(1);
    }
}
```

### slide-rs/cli/src/lib.rs
```rust
pub mod debug_sandbox;
mod exit_status;
pub mod login;
pub mod proto;

use clap::Parser;
use slide_common::CliConfigOverrides;

#[derive(Debug, Parser)]
pub struct SeatbeltCommand {
    /// Convenience alias for low-friction sandboxed automatic execution (network-disabled sandbox that can write to cwd and TMPDIR)
    #[arg(long = "full-auto", default_value_t = false)]
    pub full_auto: bool,

    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    /// Full command args to run under seatbelt.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct LandlockCommand {
    /// Convenience alias for low-friction sandboxed automatic execution (network-disabled sandbox that can write to cwd and TMPDIR)
    #[arg(long = "full-auto", default_value_t = false)]
    pub full_auto: bool,

    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,

    /// Full command args to run under landlock.
    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}
```

### slide-rs/cli/src/login.rs
```rust
use slide_common::CliConfigOverrides;
use slide_core::config::Config;
use slide_core::config::ConfigOverrides;
use slide_login::AuthMode;
use slide_login::CLIENT_ID;
use slide_login::SlideAuth;
use slide_login::OPENAI_API_KEY_ENV_VAR;
use slide_login::ServerOptions;
use slide_login::login_with_api_key;
use slide_login::logout;
use slide_login::run_login_server;
use std::env;
use std::path::PathBuf;

pub async fn login_with_chatgpt(slide_home: PathBuf) -> std::io::Result<()> {
    let opts = ServerOptions::new(slide_home, CLIENT_ID.to_string());
    let server = run_login_server(opts)?;

    eprintln!(
        "Starting local login server on http://localhost:{}.\nIf your browser did not open, navigate to this URL to authenticate:\n\n{}",
        server.actual_port, server.auth_url,
    );

    server.block_until_done().await
}

pub async fn run_login_with_chatgpt(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    match login_with_chatgpt(config.slide_home).await {
        Ok(_) => {
            eprintln!("Successfully logged in");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging in: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_login_with_api_key(
    cli_config_overrides: CliConfigOverrides,
    api_key: String,
) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    match login_with_api_key(&config.slide_home, &api_key) {
        Ok(_) => {
            eprintln!("Successfully logged in");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging in: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_login_status(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    match SlideAuth::from_slide_home(&config.slide_home, config.preferred_auth_method) {
        Ok(Some(auth)) => match auth.mode {
            AuthMode::ApiKey => match auth.get_token().await {
                Ok(api_key) => {
                    eprintln!("Logged in using an API key - {}", safe_format_key(&api_key));

                    if let Ok(env_api_key) = env::var(OPENAI_API_KEY_ENV_VAR)
                        && env_api_key == api_key
                    {
                        eprintln!(
                            "   API loaded from OPENAI_API_KEY environment variable or .env file"
                        );
                    }
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("Unexpected error retrieving API key: {e}");
                    std::process::exit(1);
                }
            },
            AuthMode::ChatGPT => {
                eprintln!("Logged in using ChatGPT");
                std::process::exit(0);
            }
        },
        Ok(None) => {
            eprintln!("Not logged in");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error checking login status: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn run_logout(cli_config_overrides: CliConfigOverrides) -> ! {
    let config = load_config_or_exit(cli_config_overrides);

    match logout(&config.slide_home) {
        Ok(true) => {
            eprintln!("Successfully logged out");
            std::process::exit(0);
        }
        Ok(false) => {
            eprintln!("Not logged in");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error logging out: {e}");
            std::process::exit(1);
        }
    }
}

fn load_config_or_exit(cli_config_overrides: CliConfigOverrides) -> Config {
    let cli_overrides = match cli_config_overrides.parse_overrides() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    let config_overrides = ConfigOverrides::default();
    match Config::load_with_cli_overrides(cli_overrides, config_overrides) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {e}");
            std::process::exit(1);
        }
    }
}

fn safe_format_key(key: &str) -> String {
    if key.len() <= 13 {
        return "***".to_string();
    }
    let prefix = &key[..8];
    let suffix = &key[key.len() - 5..];
    format!("{prefix}***{suffix}")
}

#[cfg(test)]
mod tests {
    use super::safe_format_key;

    #[test]
    fn formats_long_key() {
        let key = "sk-proj-1234567890ABCDE";
        assert_eq!(safe_format_key(key), "sk-proj-***ABCDE");
    }

    #[test]
    fn short_key_returns_stars() {
        let key = "sk-proj-12345";
        assert_eq!(safe_format_key(key), "***");
    }
}
```

### slide-rs/cli/src/proto.rs
```rust
use std::io::IsTerminal;

use clap::Parser;
use slide_common::CliConfigOverrides;
use slide_core::ConversationManager;
use slide_core::NewConversation;
use slide_core::config::Config;
use slide_core::config::ConfigOverrides;
use slide_core::protocol::Event;
use slide_core::protocol::EventMsg;
use slide_core::protocol::Submission;
use slide_login::AuthManager;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tracing::error;
use tracing::info;

#[derive(Debug, Parser)]
pub struct ProtoCli {
    #[clap(skip)]
    pub config_overrides: CliConfigOverrides,
}

pub async fn run_main(opts: ProtoCli) -> anyhow::Result<()> {
    if std::io::stdin().is_terminal() {
        anyhow::bail!("Protocol mode expects stdin to be a pipe, not a terminal");
    }

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let ProtoCli { config_overrides } = opts;
    let overrides_vec = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;

    let config = Config::load_with_cli_overrides(overrides_vec, ConfigOverrides::default())?;
    // Use conversation_manager API to start a conversation
    let conversation_manager = ConversationManager::new(AuthManager::shared(
        config.slide_home.clone(),
        config.preferred_auth_method,
    ));
    let NewConversation {
        conversation_id: _,
        conversation,
        session_configured,
    } = conversation_manager.new_conversation(config).await?;

    // Simulate streaming the session_configured event.
    let synthetic_event = Event {
        // Fake id value.
        id: "".to_string(),
        msg: EventMsg::SessionConfigured(session_configured),
    };
    let session_configured_event = match serde_json::to_string(&synthetic_event) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to serialize session_configured: {e}");
            return Err(anyhow::Error::from(e));
        }
    };
    println!("{session_configured_event}");

    // Task that reads JSON lines from stdin and forwards to Submission Queue
    let sq_fut = {
        let conversation = conversation.clone();
        async move {
            let stdin = BufReader::new(tokio::io::stdin());
            let mut lines = stdin.lines();
            loop {
                let result = tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        break
                    },
                    res = lines.next_line() => res,
                };

                match result {
                    Ok(Some(line)) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<Submission>(line) {
                            Ok(sub) => {
                                if let Err(e) = conversation.submit_with_id(sub).await {
                                    error!("{e:#}");
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("invalid submission: {e}");
                            }
                        }
                    }
                    _ => {
                        info!("Submission queue closed");
                        break;
                    }
                }
            }
        }
    };

    // Task that reads events from the agent and prints them as JSON lines to stdout
    let eq_fut = async move {
        loop {
            let event = tokio::select! {
                _ = tokio::signal::ctrl_c() => break,
                event = conversation.next_event() => event,
            };
            match event {
                Ok(event) => {
                    let event_str = match serde_json::to_string(&event) {
                        Ok(s) => s,
                        Err(e) => {
                            error!("Failed to serialize event: {e}");
                            continue;
                        }
                    };
                    println!("{event_str}");
                }
                Err(e) => {
                    error!("{e:#}");
                    break;
                }
            }
        }
        info!("Event queue closed");
    };

    tokio::join!(sq_fut, eq_fut);
    Ok(())
}
```

### slide-rs/common/src/approval_mode_cli_arg.rs
```rust
//! Standard type to use with the `--approval-mode` CLI option.
//! Available when the `cli` feature is enabled for the crate.

use clap::ValueEnum;

use slide_core::protocol::AskForApproval;

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum ApprovalModeCliArg {
    /// Only run "trusted" commands (e.g. ls, cat, sed) without asking for user
    /// approval. Will escalate to the user if the model proposes a command that
    /// is not in the "trusted" set.
    Untrusted,

    /// Run all commands without asking for user approval.
    /// Only asks for approval if a command fails to execute, in which case it
    /// will escalate to the user to ask for un-sandboxed execution.
    OnFailure,

    /// The model decides when to ask the user for approval.
    OnRequest,

    /// Never ask for user approval
    /// Execution failures are immediately returned to the model.
    Never,
}

impl From<ApprovalModeCliArg> for AskForApproval {
    fn from(value: ApprovalModeCliArg) -> Self {
        match value {
            ApprovalModeCliArg::Untrusted => AskForApproval::UnlessTrusted,
            ApprovalModeCliArg::OnFailure => AskForApproval::OnFailure,
            ApprovalModeCliArg::OnRequest => AskForApproval::OnRequest,
            ApprovalModeCliArg::Never => AskForApproval::Never,
        }
    }
}
```

### slide-rs/common/src/approval_presets.rs
```rust
use slide_core::protocol::AskForApproval;
use slide_core::protocol::SandboxPolicy;

/// A simple preset pairing an approval policy with a sandbox policy.
#[derive(Debug, Clone)]
pub struct ApprovalPreset {
    /// Stable identifier for the preset.
    pub id: &'static str,
    /// Display label shown in UIs.
    pub label: &'static str,
    /// Short human description shown next to the label in UIs.
    pub description: &'static str,
    /// Approval policy to apply.
    pub approval: AskForApproval,
    /// Sandbox policy to apply.
    pub sandbox: SandboxPolicy,
}

/// Built-in list of approval presets that pair approval and sandbox policy.
///
/// Keep this UI-agnostic so it can be reused by both TUI and MCP server.
pub fn builtin_approval_presets() -> Vec<ApprovalPreset> {
    vec![
        ApprovalPreset {
            id: "read-only",
            label: "Read Only",
            description: "Slide can read files and answer questions. Slide requires approval to make edits, run commands, or access network",
            approval: AskForApproval::OnRequest,
            sandbox: SandboxPolicy::ReadOnly,
        },
        ApprovalPreset {
            id: "auto",
            label: "Auto",
            description: "Slide can read files, make edits, and run commands in the workspace. Slide requires approval to work outside the workspace or access network",
            approval: AskForApproval::OnRequest,
            sandbox: SandboxPolicy::new_workspace_write_policy(),
        },
        ApprovalPreset {
            id: "full-access",
            label: "Full Access",
            description: "Slide can read files, make edits, and run commands with network access, without approval. Exercise caution",
            approval: AskForApproval::Never,
            sandbox: SandboxPolicy::DangerFullAccess,
        },
    ]
}
```

### slide-rs/common/src/config_override.rs
```rust
//! Support for `-c key=value` overrides shared across Slide CLI tools.
//!
//! This module provides a [`CliConfigOverrides`] struct that can be embedded
//! into a `clap`-derived CLI struct using `#[clap(flatten)]`. Each occurrence
//! of `-c key=value` (or `--config key=value`) will be collected as a raw
//! string. Helper methods are provided to convert the raw strings into
//! key/value pairs as well as to apply them onto a mutable
//! `serde_json::Value` representing the configuration tree.

use clap::ArgAction;
use clap::Parser;
use serde::de::Error as SerdeError;
use toml::Value;

/// CLI option that captures arbitrary configuration overrides specified as
/// `-c key=value`. It intentionally keeps both halves **unparsed** so that the
/// calling code can decide how to interpret the right-hand side.
#[derive(Parser, Debug, Default, Clone)]
pub struct CliConfigOverrides {
    /// Override a configuration value that would otherwise be loaded from
    /// `~/.slide/config.toml`. Use a dotted path (`foo.bar.baz`) to override
    /// nested values. The `value` portion is parsed as JSON. If it fails to
    /// parse as JSON, the raw string is used as a literal.
    ///
    /// Examples:
    ///   - `-c model="o3"`
    ///   - `-c 'sandbox_permissions=["disk-full-read-access"]'`
    ///   - `-c shell_environment_policy.inherit=all`
    #[arg(
        short = 'c',
        long = "config",
        value_name = "key=value",
        action = ArgAction::Append,
        global = true,
    )]
    pub raw_overrides: Vec<String>,
}

impl CliConfigOverrides {
    /// Parse the raw strings captured from the CLI into a list of `(path,
    /// value)` tuples where `value` is a `serde_json::Value`.
    pub fn parse_overrides(&self) -> Result<Vec<(String, Value)>, String> {
        self.raw_overrides
            .iter()
            .map(|s| {
                // Only split on the *first* '=' so values are free to contain
                // the character.
                let mut parts = s.splitn(2, '=');
                let key = match parts.next() {
                    Some(k) => k.trim(),
                    None => return Err("Override missing key".to_string()),
                };
                let value_str = parts
                    .next()
                    .ok_or_else(|| format!("Invalid override (missing '='): {s}"))?
                    .trim();

                if key.is_empty() {
                    return Err(format!("Empty key in override: {s}"));
                }

                // Attempt to parse as JSON. If that fails, treat it as a raw
                // string. This allows convenient usage such as
                // `-c model=o3` without the quotes.
                let value: Value = match parse_toml_value(value_str) {
                    Ok(v) => v,
                    Err(_) => {
                        // Strip leading/trailing quotes if present
                        let trimmed = value_str.trim().trim_matches(|c| c == '"' || c == '\'');
                        Value::String(trimmed.to_string())
                    }
                };

                Ok((key.to_string(), value))
            })
            .collect()
    }

    /// Apply all parsed overrides onto `target`. Intermediate objects will be
    /// created as necessary. Values located at the destination path will be
    /// replaced.
    pub fn apply_on_value(&self, target: &mut Value) -> Result<(), String> {
        let overrides = self.parse_overrides()?;
        for (path, value) in overrides {
            apply_single_override(target, &path, value);
        }
        Ok(())
    }
}

/// Apply a single override onto `root`, creating intermediate objects as
/// necessary.
fn apply_single_override(root: &mut Value, path: &str, value: Value) {
    use toml::value::Table;

    let parts: Vec<&str> = path.split('.').collect();
    let mut current = root;

    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;

        if is_last {
            match current {
                Value::Table(tbl) => {
                    tbl.insert((*part).to_string(), value);
                }
                _ => {
                    let mut tbl = Table::new();
                    tbl.insert((*part).to_string(), value);
                    *current = Value::Table(tbl);
                }
            }
            return;
        }

        // Traverse or create intermediate table.
        match current {
            Value::Table(tbl) => {
                current = tbl
                    .entry((*part).to_string())
                    .or_insert_with(|| Value::Table(Table::new()));
            }
            _ => {
                *current = Value::Table(Table::new());
                if let Value::Table(tbl) = current {
                    current = tbl
                        .entry((*part).to_string())
                        .or_insert_with(|| Value::Table(Table::new()));
                }
            }
        }
    }
}

fn parse_toml_value(raw: &str) -> Result<Value, toml::de::Error> {
    let wrapped = format!("_x_ = {raw}");
    let table: toml::Table = toml::from_str(&wrapped)?;
    table
        .get("_x_")
        .cloned()
        .ok_or_else(|| SerdeError::custom("missing sentinel key"))
}

#[cfg(all(test, feature = "cli"))]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_scalar() {
        let v = parse_toml_value("42").expect("parse");
        assert_eq!(v.as_integer(), Some(42));
    }

    #[test]
    fn fails_on_unquoted_string() {
        assert!(parse_toml_value("hello").is_err());
    }

    #[test]
    fn parses_array() {
        let v = parse_toml_value("[1, 2, 3]").expect("parse");
        let arr = v.as_array().expect("array");
        assert_eq!(arr.len(), 3);
    }

    #[test]
    fn parses_inline_table() {
        let v = parse_toml_value("{a = 1, b = 2}").expect("parse");
        let tbl = v.as_table().expect("table");
        assert_eq!(tbl.get("a").unwrap().as_integer(), Some(1));
        assert_eq!(tbl.get("b").unwrap().as_integer(), Some(2));
    }
}
```

### slide-rs/common/src/config_summary.rs
```rust
use slide_core::WireApi;
use slide_core::config::Config;

use crate::sandbox_summary::summarize_sandbox_policy;

/// Build a list of key/value pairs summarizing the effective configuration.
pub fn create_config_summary_entries(config: &Config) -> Vec<(&'static str, String)> {
    let mut entries = vec![
        ("workdir", config.cwd.display().to_string()),
        ("model", config.model.clone()),
        ("provider", config.model_provider_id.clone()),
        ("approval", config.approval_policy.to_string()),
        ("sandbox", summarize_sandbox_policy(&config.sandbox_policy)),
    ];
    if config.model_provider.wire_api == WireApi::Responses
        && config.model_family.supports_reasoning_summaries
    {
        entries.push((
            "reasoning effort",
            config.model_reasoning_effort.to_string(),
        ));
        entries.push((
            "reasoning summaries",
            config.model_reasoning_summary.to_string(),
        ));
    }

    entries
}
```

### slide-rs/common/src/elapsed.rs
```rust
use std::time::Duration;
use std::time::Instant;

/// Returns a string representing the elapsed time since `start_time` like
/// "1m15s" or "1.50s".
pub fn format_elapsed(start_time: Instant) -> String {
    format_duration(start_time.elapsed())
}

/// Convert a [`std::time::Duration`] into a human-readable, compact string.
///
/// Formatting rules:
/// * < 1 s  ->  "{milli}ms"
/// * < 60 s ->  "{sec:.2}s" (two decimal places)
/// * >= 60 s ->  "{min}m{sec:02}s"
pub fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis() as i64;
    format_elapsed_millis(millis)
}

fn format_elapsed_millis(millis: i64) -> String {
    if millis < 1000 {
        format!("{millis}ms")
    } else if millis < 60_000 {
        format!("{:.2}s", millis as f64 / 1000.0)
    } else {
        let minutes = millis / 60_000;
        let seconds = (millis % 60_000) / 1000;
        format!("{minutes}m{seconds:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_subsecond() {
        // Durations < 1s should be rendered in milliseconds with no decimals.
        let dur = Duration::from_millis(250);
        assert_eq!(format_duration(dur), "250ms");

        // Exactly zero should still work.
        let dur_zero = Duration::from_millis(0);
        assert_eq!(format_duration(dur_zero), "0ms");
    }

    #[test]
    fn test_format_duration_seconds() {
        // Durations between 1s (inclusive) and 60s (exclusive) should be
        // printed with 2-decimal-place seconds.
        let dur = Duration::from_millis(1_500); // 1.5s
        assert_eq!(format_duration(dur), "1.50s");

        // 59.999s rounds to 60.00s
        let dur2 = Duration::from_millis(59_999);
        assert_eq!(format_duration(dur2), "60.00s");
    }

    #[test]
    fn test_format_duration_minutes() {
        // Durations ≥ 1 minute should be printed mmss.
        let dur = Duration::from_millis(75_000); // 1m15s
        assert_eq!(format_duration(dur), "1m15s");

        let dur_exact = Duration::from_millis(60_000); // 1m0s
        assert_eq!(format_duration(dur_exact), "1m00s");

        let dur_long = Duration::from_millis(3_601_000);
        assert_eq!(format_duration(dur_long), "60m01s");
    }
}
```

### slide-rs/common/src/fuzzy_match.rs
```rust
/// Simple case-insensitive subsequence matcher used for fuzzy filtering.
///
/// Returns the indices (character positions) of the matched characters in the
/// ORIGINAL `haystack` string and a score where smaller is better.
///
/// Unicode correctness: we perform the match on a lowercased copy of the
/// haystack and needle but maintain a mapping from each character in the
/// lowercased haystack back to the original character index in `haystack`.
/// This ensures the returned indices can be safely used with
/// `str::chars().enumerate()` consumers for highlighting, even when
/// lowercasing expands certain characters (e.g., ß → ss, İ → i̇).
pub fn fuzzy_match(haystack: &str, needle: &str) -> Option<(Vec<usize>, i32)> {
    if needle.is_empty() {
        return Some((Vec::new(), i32::MAX));
    }

    let mut lowered_chars: Vec<char> = Vec::new();
    let mut lowered_to_orig_char_idx: Vec<usize> = Vec::new();
    for (orig_idx, ch) in haystack.chars().enumerate() {
        for lc in ch.to_lowercase() {
            lowered_chars.push(lc);
            lowered_to_orig_char_idx.push(orig_idx);
        }
    }

    let lowered_needle: Vec<char> = needle.to_lowercase().chars().collect();

    let mut result_orig_indices: Vec<usize> = Vec::with_capacity(lowered_needle.len());
    let mut last_lower_pos: Option<usize> = None;
    let mut cur = 0usize;
    for &nc in lowered_needle.iter() {
        let mut found_at: Option<usize> = None;
        while cur < lowered_chars.len() {
            if lowered_chars[cur] == nc {
                found_at = Some(cur);
                cur += 1;
                break;
            }
            cur += 1;
        }
        let pos = found_at?;
        result_orig_indices.push(lowered_to_orig_char_idx[pos]);
        last_lower_pos = Some(pos);
    }

    let first_lower_pos = if result_orig_indices.is_empty() {
        0usize
    } else {
        let target_orig = result_orig_indices[0];
        lowered_to_orig_char_idx
            .iter()
            .position(|&oi| oi == target_orig)
            .unwrap_or(0)
    };
    // last defaults to first for single-hit; score = extra span between first/last hit
    // minus needle len (≥0).
    // Strongly reward prefix matches by subtracting 100 when the first hit is at index 0.
    let last_lower_pos = last_lower_pos.unwrap_or(first_lower_pos);
    let window =
        (last_lower_pos as i32 - first_lower_pos as i32 + 1) - (lowered_needle.len() as i32);
    let mut score = window.max(0);
    if first_lower_pos == 0 {
        score -= 100;
    }

    result_orig_indices.sort_unstable();
    result_orig_indices.dedup();
    Some((result_orig_indices, score))
}

/// Convenience wrapper to get only the indices for a fuzzy match.
pub fn fuzzy_indices(haystack: &str, needle: &str) -> Option<Vec<usize>> {
    fuzzy_match(haystack, needle).map(|(mut idx, _)| {
        idx.sort_unstable();
        idx.dedup();
        idx
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_basic_indices() {
        let (idx, score) = match fuzzy_match("hello", "hl") {
            Some(v) => v,
            None => panic!("expected a match"),
        };
        assert_eq!(idx, vec![0, 2]);
        // 'h' at 0, 'l' at 2 -> window 1; start-of-string bonus applies (-100)
        assert_eq!(score, -99);
    }

    #[test]
    fn unicode_dotted_i_istanbul_highlighting() {
        let (idx, score) = match fuzzy_match("İstanbul", "is") {
            Some(v) => v,
            None => panic!("expected a match"),
        };
        assert_eq!(idx, vec![0, 1]);
        // Matches at lowered positions 0 and 2 -> window 1; start-of-string bonus applies
        assert_eq!(score, -99);
    }

    #[test]
    fn unicode_german_sharp_s_casefold() {
        assert!(fuzzy_match("straße", "strasse").is_none());
    }

    #[test]
    fn prefer_contiguous_match_over_spread() {
        let (_idx_a, score_a) = match fuzzy_match("abc", "abc") {
            Some(v) => v,
            None => panic!("expected a match"),
        };
        let (_idx_b, score_b) = match fuzzy_match("a-b-c", "abc") {
            Some(v) => v,
            None => panic!("expected a match"),
        };
        // Contiguous window -> 0; start-of-string bonus -> -100
        assert_eq!(score_a, -100);
        // Spread over 5 chars for 3-letter needle -> window 2; with bonus -> -98
        assert_eq!(score_b, -98);
        assert!(score_a < score_b);
    }

    #[test]
    fn start_of_string_bonus_applies() {
        let (_idx_a, score_a) = match fuzzy_match("file_name", "file") {
            Some(v) => v,
            None => panic!("expected a match"),
        };
        let (_idx_b, score_b) = match fuzzy_match("my_file_name", "file") {
            Some(v) => v,
            None => panic!("expected a match"),
        };
        // Start-of-string contiguous -> window 0; bonus -> -100
        assert_eq!(score_a, -100);
        // Non-prefix contiguous -> window 0; no bonus -> 0
        assert_eq!(score_b, 0);
        assert!(score_a < score_b);
    }

    #[test]
    fn empty_needle_matches_with_max_score_and_no_indices() {
        let (idx, score) = match fuzzy_match("anything", "") {
            Some(v) => v,
            None => panic!("empty needle should match"),
        };
        assert!(idx.is_empty());
        assert_eq!(score, i32::MAX);
    }

    #[test]
    fn case_insensitive_matching_basic() {
        let (idx, score) = match fuzzy_match("FooBar", "foO") {
            Some(v) => v,
            None => panic!("expected a match"),
        };
        assert_eq!(idx, vec![0, 1, 2]);
        // Contiguous prefix match (case-insensitive) -> window 0 with bonus
        assert_eq!(score, -100);
    }

    #[test]
    fn indices_are_deduped_for_multichar_lowercase_expansion() {
        let needle = "\u{0069}\u{0307}"; // "i" + combining dot above
        let (idx, score) = match fuzzy_match("İ", needle) {
            Some(v) => v,
            None => panic!("expected a match"),
        };
        assert_eq!(idx, vec![0]);
        // Lowercasing 'İ' expands to two chars; contiguous prefix -> window 0 with bonus
        assert_eq!(score, -100);
    }
}
```

### slide-rs/common/src/lib.rs
```rust
#[cfg(feature = "cli")]
mod approval_mode_cli_arg;

#[cfg(feature = "elapsed")]
pub mod elapsed;

#[cfg(feature = "cli")]
pub use approval_mode_cli_arg::ApprovalModeCliArg;

#[cfg(feature = "cli")]
mod sandbox_mode_cli_arg;

#[cfg(feature = "cli")]
pub use sandbox_mode_cli_arg::SandboxModeCliArg;

#[cfg(any(feature = "cli", test))]
mod config_override;

#[cfg(feature = "cli")]
pub use config_override::CliConfigOverrides;

mod sandbox_summary;

#[cfg(feature = "sandbox_summary")]
pub use sandbox_summary::summarize_sandbox_policy;

mod config_summary;

pub use config_summary::create_config_summary_entries;
// Shared fuzzy matcher (used by TUI selection popups and other UI filtering)
pub mod fuzzy_match;
// Shared model presets used by TUI and MCP server
pub mod model_presets;
// Shared approval presets (AskForApproval + Sandbox) used by TUI and MCP server
// Not to be confused with AskForApproval, which we should probably rename to EscalationPolicy.
pub mod approval_presets;
```

### slide-rs/common/src/model_presets.rs
```rust
use slide_core::protocol_config_types::ReasoningEffort;

/// A simple preset pairing a model slug with a reasoning effort.
#[derive(Debug, Clone, Copy)]
pub struct ModelPreset {
    /// Stable identifier for the preset.
    pub id: &'static str,
    /// Display label shown in UIs.
    pub label: &'static str,
    /// Short human description shown next to the label in UIs.
    pub description: &'static str,
    /// Model slug (e.g., "gpt-5").
    pub model: &'static str,
    /// Reasoning effort to apply for this preset.
    pub effort: ReasoningEffort,
}

/// Built-in list of model presets that pair a model with a reasoning effort.
///
/// Keep this UI-agnostic so it can be reused by both TUI and MCP server.
pub fn builtin_model_presets() -> &'static [ModelPreset] {
    // Order reflects effort from minimal to high.
    const PRESETS: &[ModelPreset] = &[
        ModelPreset {
            id: "gpt-5-minimal",
            label: "gpt-5 minimal",
            description: "— fastest responses with limited reasoning; ideal for coding, instructions, or lightweight tasks",
            model: "gpt-5",
            effort: ReasoningEffort::Minimal,
        },
        ModelPreset {
            id: "gpt-5-low",
            label: "gpt-5 low",
            description: "— balances speed with some reasoning; useful for straightforward queries and short explanations",
            model: "gpt-5",
            effort: ReasoningEffort::Low,
        },
        ModelPreset {
            id: "gpt-5-medium",
            label: "gpt-5 medium",
            description: "— default setting; provides a solid balance of reasoning depth and latency for general-purpose tasks",
            model: "gpt-5",
            effort: ReasoningEffort::Medium,
        },
        ModelPreset {
            id: "gpt-5-high",
            label: "gpt-5 high",
            description: "— maximizes reasoning depth for complex or ambiguous problems",
            model: "gpt-5",
            effort: ReasoningEffort::High,
        },
    ];
    PRESETS
}
```

### slide-rs/common/src/sandbox_mode_cli_arg.rs
```rust
//! Standard type to use with the `--sandbox` (`-s`) CLI option.
//!
//! This mirrors the variants of [`slide_core::protocol::SandboxPolicy`], but
//! without any of the associated data so it can be expressed as a simple flag
//! on the command-line. Users that need to tweak the advanced options for
//! `workspace-write` can continue to do so via `-c` overrides or their
//! `config.toml`.

use clap::ValueEnum;
use slide_protocol::config_types::SandboxMode;

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum SandboxModeCliArg {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl From<SandboxModeCliArg> for SandboxMode {
    fn from(value: SandboxModeCliArg) -> Self {
        match value {
            SandboxModeCliArg::ReadOnly => SandboxMode::ReadOnly,
            SandboxModeCliArg::WorkspaceWrite => SandboxMode::WorkspaceWrite,
            SandboxModeCliArg::DangerFullAccess => SandboxMode::DangerFullAccess,
        }
    }
}
```

### slide-rs/common/src/sandbox_summary.rs
```rust
use slide_core::protocol::SandboxPolicy;

pub fn summarize_sandbox_policy(sandbox_policy: &SandboxPolicy) -> String {
    match sandbox_policy {
        SandboxPolicy::DangerFullAccess => "danger-full-access".to_string(),
        SandboxPolicy::ReadOnly => "read-only".to_string(),
        SandboxPolicy::WorkspaceWrite {
            writable_roots,
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        } => {
            let mut summary = "workspace-write".to_string();

            let mut writable_entries = Vec::<String>::new();
            writable_entries.push("workdir".to_string());
            if !*exclude_slash_tmp {
                writable_entries.push("/tmp".to_string());
            }
            if !*exclude_tmpdir_env_var {
                writable_entries.push("$TMPDIR".to_string());
            }
            writable_entries.extend(
                writable_roots
                    .iter()
                    .map(|p| p.to_string_lossy().to_string()),
            );

            summary.push_str(&format!(" [{}]", writable_entries.join(", ")));
            if *network_access {
                summary.push_str(" (network access enabled)");
            }
            summary
        }
    }
}
```

### slide-rs/core/src/seatbelt.rs
```rust
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

use crate::protocol::SandboxPolicy;
use crate::spawn::SLIDE_SANDBOX_ENV_VAR;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;

const MACOS_SEATBELT_BASE_POLICY: &str = include_str!("seatbelt_base_policy.sbpl");

/// When working with `sandbox-exec`, only consider `sandbox-exec` in `/usr/bin`
/// to defend against an attacker trying to inject a malicious version on the
/// PATH. If /usr/bin/sandbox-exec has been tampered with, then the attacker
/// already has root access.
const MACOS_PATH_TO_SEATBELT_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

pub async fn spawn_command_under_seatbelt(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: PathBuf,
    stdio_policy: StdioPolicy,
    mut env: HashMap<String, String>,
) -> std::io::Result<Child> {
    let args = create_seatbelt_command_args(command, sandbox_policy, &cwd);
    let arg0 = None;
    env.insert(SLIDE_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
    spawn_child_async(
        PathBuf::from(MACOS_PATH_TO_SEATBELT_EXECUTABLE),
        args,
        arg0,
        cwd,
        sandbox_policy,
        stdio_policy,
        env,
    )
    .await
}

fn create_seatbelt_command_args(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> Vec<String> {
    let (file_write_policy, extra_cli_args) = {
        if sandbox_policy.has_full_disk_write_access() {
            // Allegedly, this is more permissive than `(allow file-write*)`.
            (
                r#"(allow file-write* (regex #"^/"))"#.to_string(),
                Vec::<String>::new(),
            )
        } else {
            let writable_roots = sandbox_policy.get_writable_roots_with_cwd(cwd);

            let mut writable_folder_policies: Vec<String> = Vec::new();
            let mut cli_args: Vec<String> = Vec::new();

            for (index, wr) in writable_roots.iter().enumerate() {
                // Canonicalize to avoid mismatches like /var vs /private/var on macOS.
                let canonical_root = wr.root.canonicalize().unwrap_or_else(|_| wr.root.clone());
                let root_param = format!("WRITABLE_ROOT_{index}");
                cli_args.push(format!(
                    "-D{root_param}={}",
                    canonical_root.to_string_lossy()
                ));

                if wr.read_only_subpaths.is_empty() {
                    writable_folder_policies.push(format!("(subpath (param \"{root_param}\"))"));
                } else {
                    // Add parameters for each read-only subpath and generate
                    // the `(require-not ...)` clauses.
                    let mut require_parts: Vec<String> = Vec::new();
                    require_parts.push(format!("(subpath (param \"{root_param}\"))"));
                    for (subpath_index, ro) in wr.read_only_subpaths.iter().enumerate() {
                        let canonical_ro = ro.canonicalize().unwrap_or_else(|_| ro.clone());
                        let ro_param = format!("WRITABLE_ROOT_{index}_RO_{subpath_index}");
                        cli_args.push(format!("-D{ro_param}={}", canonical_ro.to_string_lossy()));
                        require_parts
                            .push(format!("(require-not (subpath (param \"{ro_param}\")))"));
                    }
                    let policy_component = format!("(require-all {} )", require_parts.join(" "));
                    writable_folder_policies.push(policy_component);
                }
            }

            if writable_folder_policies.is_empty() {
                ("".to_string(), Vec::<String>::new())
            } else {
                let file_write_policy = format!(
                    "(allow file-write*\n{}\n)",
                    writable_folder_policies.join(" ")
                );
                (file_write_policy, cli_args)
            }
        }
    };

    let file_read_policy = if sandbox_policy.has_full_disk_read_access() {
        "; allow read-only file operations\n(allow file-read*)"
    } else {
        ""
    };

    // TODO(mbolin): apply_patch calls must also honor the SandboxPolicy.
    let network_policy = if sandbox_policy.has_full_network_access() {
        "(allow network-outbound)\n(allow network-inbound)\n(allow system-socket)"
    } else {
        ""
    };

    let full_policy = format!(
        "{MACOS_SEATBELT_BASE_POLICY}\n{file_read_policy}\n{file_write_policy}\n{network_policy}"
    );

    let mut seatbelt_args: Vec<String> = vec!["-p".to_string(), full_policy];
    seatbelt_args.extend(extra_cli_args);
    seatbelt_args.push("--".to_string());
    seatbelt_args.extend(command);
    seatbelt_args
}

#[cfg(test)]
mod tests {
    use super::MACOS_SEATBELT_BASE_POLICY;
    use super::create_seatbelt_command_args;
    use crate::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn create_seatbelt_args_with_read_only_git_subpath() {
        if cfg!(target_os = "windows") {
            // /tmp does not exist on Windows, so skip this test.
            return;
        }

        // Create a temporary workspace with two writable roots: one containing
        // a top-level .git directory and one without it.
        let tmp = TempDir::new().expect("tempdir");
        let PopulatedTmp {
            root_with_git,
            root_without_git,
            root_with_git_canon,
            root_with_git_git_canon,
            root_without_git_canon,
        } = populate_tmpdir(tmp.path());
        let cwd = tmp.path().join("cwd");

        // Build a policy that only includes the two test roots as writable and
        // does not automatically include defaults TMPDIR or /tmp.
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![root_with_git.clone(), root_without_git.clone()],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let args = create_seatbelt_command_args(
            vec!["/bin/echo".to_string(), "hello".to_string()],
            &policy,
            &cwd,
        );

        // Build the expected policy text using a raw string for readability.
        // Note that the policy includes:
        // - the base policy,
        // - read-only access to the filesystem,
        // - write access to WRITABLE_ROOT_0 (but not its .git) and WRITABLE_ROOT_1.
        let expected_policy = format!(
            r#"{MACOS_SEATBELT_BASE_POLICY}
; allow read-only file operations
(allow file-read*)
(allow file-write*
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) ) (subpath (param "WRITABLE_ROOT_1")) (subpath (param "WRITABLE_ROOT_2"))
)
"#,
        );

        let mut expected_args = vec![
            "-p".to_string(),
            expected_policy,
            format!(
                "-DWRITABLE_ROOT_0={}",
                root_with_git_canon.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_0={}",
                root_with_git_git_canon.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_1={}",
                root_without_git_canon.to_string_lossy()
            ),
            format!("-DWRITABLE_ROOT_2={}", cwd.to_string_lossy()),
        ];

        expected_args.extend(vec![
            "--".to_string(),
            "/bin/echo".to_string(),
            "hello".to_string(),
        ]);

        assert_eq!(expected_args, args);
    }

    #[test]
    fn create_seatbelt_args_for_cwd_as_git_repo() {
        if cfg!(target_os = "windows") {
            // /tmp does not exist on Windows, so skip this test.
            return;
        }

        // Create a temporary workspace with two writable roots: one containing
        // a top-level .git directory and one without it.
        let tmp = TempDir::new().expect("tempdir");
        let PopulatedTmp {
            root_with_git,
            root_with_git_canon,
            root_with_git_git_canon,
            ..
        } = populate_tmpdir(tmp.path());

        // Build a policy that does not specify any writable_roots, but does
        // use the default ones (cwd and TMPDIR) and verifies the `.git` check
        // is done properly for cwd.
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

        let args = create_seatbelt_command_args(
            vec!["/bin/echo".to_string(), "hello".to_string()],
            &policy,
            root_with_git.as_path(),
        );

        let tmpdir_env_var = std::env::var("TMPDIR")
            .ok()
            .map(PathBuf::from)
            .and_then(|p| p.canonicalize().ok())
            .map(|p| p.to_string_lossy().to_string());

        let tempdir_policy_entry = if tmpdir_env_var.is_some() {
            r#" (subpath (param "WRITABLE_ROOT_2"))"#
        } else {
            ""
        };

        // Build the expected policy text using a raw string for readability.
        // Note that the policy includes:
        // - the base policy,
        // - read-only access to the filesystem,
        // - write access to WRITABLE_ROOT_0 (but not its .git) and WRITABLE_ROOT_1.
        let expected_policy = format!(
            r#"{MACOS_SEATBELT_BASE_POLICY}
; allow read-only file operations
(allow file-read*)
(allow file-write*
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) ) (subpath (param "WRITABLE_ROOT_1")){tempdir_policy_entry}
)
"#,
        );

        let mut expected_args = vec![
            "-p".to_string(),
            expected_policy,
            format!(
                "-DWRITABLE_ROOT_0={}",
                root_with_git_canon.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_0={}",
                root_with_git_git_canon.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_1={}",
                PathBuf::from("/tmp")
                    .canonicalize()
                    .expect("canonicalize /tmp")
                    .to_string_lossy()
            ),
        ];

        if let Some(p) = tmpdir_env_var {
            expected_args.push(format!("-DWRITABLE_ROOT_2={p}"));
        }

        expected_args.extend(vec![
            "--".to_string(),
            "/bin/echo".to_string(),
            "hello".to_string(),
        ]);

        assert_eq!(expected_args, args);
    }

    struct PopulatedTmp {
        root_with_git: PathBuf,
        root_without_git: PathBuf,
        root_with_git_canon: PathBuf,
        root_with_git_git_canon: PathBuf,
        root_without_git_canon: PathBuf,
    }

    fn populate_tmpdir(tmp: &Path) -> PopulatedTmp {
        let root_with_git = tmp.join("with_git");
        let root_without_git = tmp.join("no_git");
        fs::create_dir_all(&root_with_git).expect("create with_git");
        fs::create_dir_all(&root_without_git).expect("create no_git");
        fs::create_dir_all(root_with_git.join(".git")).expect("create .git");

        // Ensure we have canonical paths for -D parameter matching.
        let root_with_git_canon = root_with_git.canonicalize().expect("canonicalize with_git");
        let root_with_git_git_canon = root_with_git_canon.join(".git");
        let root_without_git_canon = root_without_git
            .canonicalize()
            .expect("canonicalize no_git");
        PopulatedTmp {
            root_with_git,
            root_without_git,
            root_with_git_canon,
            root_with_git_git_canon,
            root_without_git_canon,
        }
    }
}
```

### slide-rs/core/src/user_notification.rs
```rust
use serde::Serialize;

/// User can configure a program that will receive notifications. Each
/// notification is serialized as JSON and passed as an argument to the
/// program.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(crate) enum UserNotification {
    #[serde(rename_all = "kebab-case")]
    AgentTurnComplete {
        turn_id: String,

        /// Messages that the user sent to the agent to initiate the turn.
        input_messages: Vec<String>,

        /// The last message sent by the assistant in the turn.
        last_assistant_message: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_notification() {
        let notification = UserNotification::AgentTurnComplete {
            turn_id: "12345".to_string(),
            input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
            last_assistant_message: Some(
                "Rename complete and verified `cargo build` succeeds.".to_string(),
            ),
        };
        let serialized = serde_json::to_string(&notification).unwrap();
        assert_eq!(
            serialized,
            r#"{"type":"agent-turn-complete","turn-id":"12345","input-messages":["Rename `foo` to `bar` and update the callsites."],"last-assistant-message":"Rename complete and verified `cargo build` succeeds."}"#
        );
    }
}
```

### slide-rs/core/src/flags.rs
```rust
use std::time::Duration;

use env_flags::env_flags;

env_flags! {
    pub OPENAI_API_BASE: &str = "https://api.openai.com/v1";

    /// Fallback when the provider-specific key is not set.
    pub OPENAI_API_KEY: Option<&str> = None;
    pub OPENAI_TIMEOUT_MS: Duration = Duration::from_millis(300_000), |value| {
        value.parse().map(Duration::from_millis)
    };

    /// Fixture path for offline tests (see client.rs).
    pub SLIDE_RS_SSE_FIXTURE: Option<&str> = None;
}
```

### slide-rs/core/src/exec.rs
```rust
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;
use std::time::Instant;

use async_channel::Sender;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::process::Child;

use crate::error::SlideErr;
use crate::error::Result;
use crate::error::SandboxErr;
use crate::landlock::spawn_command_under_linux_sandbox;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandOutputDeltaEvent;
use crate::protocol::ExecOutputStream;
use crate::protocol::SandboxPolicy;
use crate::seatbelt::spawn_command_under_seatbelt;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use serde_bytes::ByteBuf;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;

// Hardcode these since it does not seem worth including the libc crate just
// for these.
const SIGKILL_CODE: i32 = 9;
const TIMEOUT_CODE: i32 = 64;
const EXIT_CODE_SIGNAL_BASE: i32 = 128; // conventional shell: 128 + signal

// I/O buffer sizing
const READ_CHUNK_SIZE: usize = 8192; // bytes per read
const AGGREGATE_BUFFER_INITIAL_CAPACITY: usize = 8 * 1024; // 8 KiB

/// Limit the number of ExecCommandOutputDelta events emitted per exec call.
/// Aggregation still collects full output; only the live event stream is capped.
pub(crate) const MAX_EXEC_OUTPUT_DELTAS_PER_CALL: usize = 10_000;

#[derive(Debug, Clone)]
pub struct ExecParams {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub timeout_ms: Option<u64>,
    pub env: HashMap<String, String>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
}

impl ExecParams {
    pub fn timeout_duration(&self) -> Duration {
        Duration::from_millis(self.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SandboxType {
    None,

    /// Only available on macOS.
    MacosSeatbelt,

    /// Only available on Linux.
    LinuxSeccomp,
}

#[derive(Clone)]
pub struct StdoutStream {
    pub sub_id: String,
    pub call_id: String,
    pub tx_event: Sender<Event>,
}

pub async fn process_exec_tool_call(
    params: ExecParams,
    sandbox_type: SandboxType,
    sandbox_policy: &SandboxPolicy,
    slide_linux_sandbox_exe: &Option<PathBuf>,
    stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    let start = Instant::now();

    let raw_output_result: std::result::Result<RawExecToolCallOutput, SlideErr> = match sandbox_type
    {
        SandboxType::None => exec(params, sandbox_policy, stdout_stream.clone()).await,
        SandboxType::MacosSeatbelt => {
            let timeout = params.timeout_duration();
            let ExecParams {
                command, cwd, env, ..
            } = params;
            let child = spawn_command_under_seatbelt(
                command,
                sandbox_policy,
                cwd,
                StdioPolicy::RedirectForShellTool,
                env,
            )
            .await?;
            consume_truncated_output(child, timeout, stdout_stream.clone()).await
        }
        SandboxType::LinuxSeccomp => {
            let timeout = params.timeout_duration();
            let ExecParams {
                command, cwd, env, ..
            } = params;

            let slide_linux_sandbox_exe = slide_linux_sandbox_exe
                .as_ref()
                .ok_or(SlideErr::LandlockSandboxExecutableNotProvided)?;
            let child = spawn_command_under_linux_sandbox(
                slide_linux_sandbox_exe,
                command,
                sandbox_policy,
                cwd,
                StdioPolicy::RedirectForShellTool,
                env,
            )
            .await?;

            consume_truncated_output(child, timeout, stdout_stream).await
        }
    };
    let duration = start.elapsed();
    match raw_output_result {
        Ok(raw_output) => {
            let stdout = raw_output.stdout.from_utf8_lossy();
            let stderr = raw_output.stderr.from_utf8_lossy();

            #[cfg(target_family = "unix")]
            match raw_output.exit_status.signal() {
                Some(TIMEOUT_CODE) => return Err(SlideErr::Sandbox(SandboxErr::Timeout)),
                Some(signal) => {
                    return Err(SlideErr::Sandbox(SandboxErr::Signal(signal)));
                }
                None => {}
            }

            let exit_code = raw_output.exit_status.code().unwrap_or(-1);

            if exit_code != 0 && is_likely_sandbox_denied(sandbox_type, exit_code) {
                return Err(SlideErr::Sandbox(SandboxErr::Denied(
                    exit_code,
                    stdout.text,
                    stderr.text,
                )));
            }

            Ok(ExecToolCallOutput {
                exit_code,
                stdout,
                stderr,
                aggregated_output: raw_output.aggregated_output.from_utf8_lossy(),
                duration,
            })
        }
        Err(err) => {
            tracing::error!("exec error: {err}");
            Err(err)
        }
    }
}

/// We don't have a fully deterministic way to tell if our command failed
/// because of the sandbox - a command in the user's zshrc file might hit an
/// error, but the command itself might fail or succeed for other reasons.
/// For now, we conservatively check for 'command not found' (exit code 127),
/// and can add additional cases as necessary.
fn is_likely_sandbox_denied(sandbox_type: SandboxType, exit_code: i32) -> bool {
    if sandbox_type == SandboxType::None {
        return false;
    }

    // Quick rejects: well-known non-sandbox shell exit codes
    // 127: command not found, 2: misuse of shell builtins
    if exit_code == 127 {
        return false;
    }

    // For all other cases, we assume the sandbox is the cause
    true
}

#[derive(Debug)]
pub struct StreamOutput<T> {
    pub text: T,
    pub truncated_after_lines: Option<u32>,
}
#[derive(Debug)]
struct RawExecToolCallOutput {
    pub exit_status: ExitStatus,
    pub stdout: StreamOutput<Vec<u8>>,
    pub stderr: StreamOutput<Vec<u8>>,
    pub aggregated_output: StreamOutput<Vec<u8>>,
}

impl StreamOutput<String> {
    pub fn new(text: String) -> Self {
        Self {
            text,
            truncated_after_lines: None,
        }
    }
}

impl StreamOutput<Vec<u8>> {
    pub fn from_utf8_lossy(&self) -> StreamOutput<String> {
        StreamOutput {
            text: String::from_utf8_lossy(&self.text).to_string(),
            truncated_after_lines: self.truncated_after_lines,
        }
    }
}

#[inline]
fn append_all(dst: &mut Vec<u8>, src: &[u8]) {
    dst.extend_from_slice(src);
}

#[derive(Debug)]
pub struct ExecToolCallOutput {
    pub exit_code: i32,
    pub stdout: StreamOutput<String>,
    pub stderr: StreamOutput<String>,
    pub aggregated_output: StreamOutput<String>,
    pub duration: Duration,
}

async fn exec(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
) -> Result<RawExecToolCallOutput> {
    let timeout = params.timeout_duration();
    let ExecParams {
        command, cwd, env, ..
    } = params;

    let (program, args) = command.split_first().ok_or_else(|| {
        SlideErr::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command args are empty",
        ))
    })?;
    let arg0 = None;
    let child = spawn_child_async(
        PathBuf::from(program),
        args.into(),
        arg0,
        cwd,
        sandbox_policy,
        StdioPolicy::RedirectForShellTool,
        env,
    )
    .await?;
    consume_truncated_output(child, timeout, stdout_stream).await
}

/// Consumes the output of a child process, truncating it so it is suitable for
/// use as the output of a `shell` tool call. Also enforces specified timeout.
async fn consume_truncated_output(
    mut child: Child,
    timeout: Duration,
    stdout_stream: Option<StdoutStream>,
) -> Result<RawExecToolCallOutput> {
    // Both stdout and stderr were configured with `Stdio::piped()`
    // above, therefore `take()` should normally return `Some`.  If it doesn't
    // we treat it as an exceptional I/O error

    let stdout_reader = child.stdout.take().ok_or_else(|| {
        SlideErr::Io(io::Error::other(
            "stdout pipe was unexpectedly not available",
        ))
    })?;
    let stderr_reader = child.stderr.take().ok_or_else(|| {
        SlideErr::Io(io::Error::other(
            "stderr pipe was unexpectedly not available",
        ))
    })?;

    let (agg_tx, agg_rx) = async_channel::unbounded::<Vec<u8>>();

    let stdout_handle = tokio::spawn(read_capped(
        BufReader::new(stdout_reader),
        stdout_stream.clone(),
        false,
        Some(agg_tx.clone()),
    ));
    let stderr_handle = tokio::spawn(read_capped(
        BufReader::new(stderr_reader),
        stdout_stream.clone(),
        true,
        Some(agg_tx.clone()),
    ));

    let exit_status = tokio::select! {
        result = tokio::time::timeout(timeout, child.wait()) => {
            match result {
                Ok(Ok(exit_status)) => exit_status,
                Ok(e) => e?,
                Err(_) => {
                    // timeout
                    child.start_kill()?;
                    // Debatable whether `child.wait().await` should be called here.
                    synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + TIMEOUT_CODE)
                }
            }
        }
        _ = tokio::signal::ctrl_c() => {
            child.start_kill()?;
            synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + SIGKILL_CODE)
        }
    };

    let stdout = stdout_handle.await??;
    let stderr = stderr_handle.await??;

    drop(agg_tx);

    let mut combined_buf = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY);
    while let Ok(chunk) = agg_rx.recv().await {
        append_all(&mut combined_buf, &chunk);
    }
    let aggregated_output = StreamOutput {
        text: combined_buf,
        truncated_after_lines: None,
    };

    Ok(RawExecToolCallOutput {
        exit_status,
        stdout,
        stderr,
        aggregated_output,
    })
}

async fn read_capped<R: AsyncRead + Unpin + Send + 'static>(
    mut reader: R,
    stream: Option<StdoutStream>,
    is_stderr: bool,
    aggregate_tx: Option<Sender<Vec<u8>>>,
) -> io::Result<StreamOutput<Vec<u8>>> {
    let mut buf = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY);
    let mut tmp = [0u8; READ_CHUNK_SIZE];
    let mut emitted_deltas: usize = 0;

    // No caps: append all bytes

    loop {
        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            break;
        }

        if let Some(stream) = &stream
            && emitted_deltas < MAX_EXEC_OUTPUT_DELTAS_PER_CALL
        {
            let chunk = tmp[..n].to_vec();
            let msg = EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                call_id: stream.call_id.clone(),
                stream: if is_stderr {
                    ExecOutputStream::Stderr
                } else {
                    ExecOutputStream::Stdout
                },
                chunk: ByteBuf::from(chunk),
            });
            let event = Event {
                id: stream.sub_id.clone(),
                msg,
            };
            #[allow(clippy::let_unit_value)]
            let _ = stream.tx_event.send(event).await;
            emitted_deltas += 1;
        }

        if let Some(tx) = &aggregate_tx {
            let _ = tx.send(tmp[..n].to_vec()).await;
        }

        append_all(&mut buf, &tmp[..n]);
        // Continue reading to EOF to avoid back-pressure
    }

    Ok(StreamOutput {
        text: buf,
        truncated_after_lines: None,
    })
}

#[cfg(unix)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code)
}

#[cfg(windows)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    #[expect(clippy::unwrap_used)]
    std::process::ExitStatus::from_raw(code.try_into().unwrap())
}
```

### slide-rs/core/src/util.rs
```rust
use std::path::Path;
use std::time::Duration;

use rand::Rng;

const INITIAL_DELAY_MS: u64 = 200;
const BACKOFF_FACTOR: f64 = 2.0;

pub(crate) fn backoff(attempt: u64) -> Duration {
    let exp = BACKOFF_FACTOR.powi(attempt.saturating_sub(1) as i32);
    let base = (INITIAL_DELAY_MS as f64 * exp) as u64;
    let jitter = rand::rng().random_range(0.9..1.1);
    Duration::from_millis((base as f64 * jitter) as u64)
}

/// Return `true` if the project folder specified by the `Config` is inside a
/// Git repository.
///
/// The check walks up the directory hierarchy looking for a `.git` file or
/// directory (note `.git` can be a file that contains a `gitdir` entry). This
/// approach does **not** require the `git` binary or the `git2` crate and is
/// therefore fairly lightweight.
///
/// Note that this does **not** detect *work‑trees* created with
/// `git worktree add` where the checkout lives outside the main repository
/// directory. If you need Slide to work from such a checkout simply pass the
/// `--allow-no-git-exec` CLI flag that disables the repo requirement.
pub fn is_inside_git_repo(base_dir: &Path) -> bool {
    let mut dir = base_dir.to_path_buf();

    loop {
        if dir.join(".git").exists() {
            return true;
        }

        // Pop one component (go up one directory).  `pop` returns false when
        // we have reached the filesystem root.
        if !dir.pop() {
            break;
        }
    }

    false
}
```

### slide-rs/core/src/openai_tools.rs
```rust
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;

use crate::model_family::ModelFamily;
use crate::plan_tool::PLAN_TOOL;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use crate::tool_apply_patch::ApplyPatchToolType;
use crate::tool_apply_patch::create_apply_patch_freeform_tool;
use crate::tool_apply_patch::create_apply_patch_json_tool;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ResponsesApiTool {
    pub(crate) name: String,
    pub(crate) description: String,
    /// TODO: Validation. When strict is set to true, the JSON schema,
    /// `required` and `additional_properties` must be present. All fields in
    /// `properties` must be present in `required`.
    pub(crate) strict: bool,
    pub(crate) parameters: JsonSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreeformTool {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) format: FreeformToolFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FreeformToolFormat {
    pub(crate) r#type: String,
    pub(crate) syntax: String,
    pub(crate) definition: String,
}

/// When serialized as JSON, this produces a valid "Tool" in the OpenAI
/// Responses API.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub(crate) enum OpenAiTool {
    #[serde(rename = "function")]
    Function(ResponsesApiTool),
    #[serde(rename = "local_shell")]
    LocalShell {},
    // TODO: Understand why we get an error on web_search although the API docs say it's supported.
    // https://platform.openai.com/docs/guides/tools-web-search?api-mode=responses#:~:text=%7B%20type%3A%20%22web_search%22%20%7D%2C
    #[serde(rename = "web_search_preview")]
    WebSearch {},
    #[serde(rename = "custom")]
    Freeform(FreeformTool),
}

#[derive(Debug, Clone)]
pub enum ConfigShellToolType {
    DefaultShell,
    ShellWithRequest { sandbox_policy: SandboxPolicy },
    LocalShell,
    StreamableShell,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolsConfig {
    pub shell_type: ConfigShellToolType,
    pub plan_tool: bool,
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
    pub web_search_request: bool,
    pub include_view_image_tool: bool,
}

pub(crate) struct ToolsConfigParams<'a> {
    pub(crate) model_family: &'a ModelFamily,
    pub(crate) approval_policy: AskForApproval,
    pub(crate) sandbox_policy: SandboxPolicy,
    pub(crate) include_plan_tool: bool,
    pub(crate) include_apply_patch_tool: bool,
    pub(crate) include_web_search_request: bool,
    pub(crate) use_streamable_shell_tool: bool,
    pub(crate) include_view_image_tool: bool,
}

impl ToolsConfig {
    pub fn new(params: &ToolsConfigParams) -> Self {
        let ToolsConfigParams {
            model_family,
            approval_policy,
            sandbox_policy,
            include_plan_tool,
            include_apply_patch_tool,
            include_web_search_request,
            use_streamable_shell_tool,
            include_view_image_tool,
        } = params;
        let mut shell_type = if *use_streamable_shell_tool {
            ConfigShellToolType::StreamableShell
        } else if model_family.uses_local_shell_tool {
            ConfigShellToolType::LocalShell
        } else {
            ConfigShellToolType::DefaultShell
        };
        if matches!(approval_policy, AskForApproval::OnRequest) && !use_streamable_shell_tool {
            shell_type = ConfigShellToolType::ShellWithRequest {
                sandbox_policy: sandbox_policy.clone(),
            }
        }

        let apply_patch_tool_type = match model_family.apply_patch_tool_type {
            Some(ApplyPatchToolType::Freeform) => Some(ApplyPatchToolType::Freeform),
            Some(ApplyPatchToolType::Function) => Some(ApplyPatchToolType::Function),
            None => {
                if *include_apply_patch_tool {
                    Some(ApplyPatchToolType::Freeform)
                } else {
                    None
                }
            }
        };

        Self {
            shell_type,
            plan_tool: *include_plan_tool,
            apply_patch_tool_type,
            web_search_request: *include_web_search_request,
            include_view_image_tool: *include_view_image_tool,
        }
    }
}

/// Generic JSON‑Schema subset needed for our tool definitions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum JsonSchema {
    Boolean {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    String {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    /// MCP schema allows "number" | "integer" for Number
    #[serde(alias = "integer")]
    Number {
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    Array {
        items: Box<JsonSchema>,

        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    Object {
        properties: BTreeMap<String, JsonSchema>,
        #[serde(skip_serializing_if = "Option::is_none")]
        required: Option<Vec<String>>,
        #[serde(
            rename = "additionalProperties",
            skip_serializing_if = "Option::is_none"
        )]
        additional_properties: Option<bool>,
    },
}

fn create_shell_tool() -> OpenAiTool {
    let mut properties = BTreeMap::new();
    properties.insert(
        "command".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String { description: None }),
            description: Some("The command to execute".to_string()),
        },
    );
    properties.insert(
        "workdir".to_string(),
        JsonSchema::String {
            description: Some("The working directory to execute the command in".to_string()),
        },
    );
    properties.insert(
        "timeout_ms".to_string(),
        JsonSchema::Number {
            description: Some("The timeout for the command in milliseconds".to_string()),
        },
    );

    OpenAiTool::Function(ResponsesApiTool {
        name: "shell".to_string(),
        description: "Runs a shell command and returns its output".to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["command".to_string()]),
            additional_properties: Some(false),
        },
    })
}

fn create_view_image_tool() -> OpenAiTool {
    // Support only local filesystem path.
    let mut properties = BTreeMap::new();
    properties.insert(
        "path".to_string(),
        JsonSchema::String {
            description: Some("Local filesystem path to an image file".to_string()),
        },
    );

    OpenAiTool::Function(ResponsesApiTool {
        name: "view_image".to_string(),
        description:
            "Attach a local image (by filesystem path) to the conversation context for this turn."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["path".to_string()]),
            additional_properties: Some(false),
        },
    })
}

/// Returns JSON values that are compatible with Function Calling in the
/// Responses API:
/// https://platform.openai.com/docs/guides/function-calling?api-mode=responses
pub fn create_tools_json_for_responses_api(
    tools: &Vec<OpenAiTool>,
) -> crate::error::Result<Vec<serde_json::Value>> {
    let mut tools_json = Vec::new();

    for tool in tools {
        let json = serde_json::to_value(tool)?;
        tools_json.push(json);
    }

    Ok(tools_json)
}

/// Returns a list of OpenAiTools based on the provided config.
pub(crate) fn get_openai_tools(
    config: &ToolsConfig,
) -> Vec<OpenAiTool> {
    let mut tools: Vec<OpenAiTool> = Vec::new();

    match &config.shell_type {
        ConfigShellToolType::DefaultShell => {
            tools.push(create_shell_tool());
        }
        ConfigShellToolType::ShellWithRequest { .. } => {
            tools.push(create_shell_tool());
        }
        ConfigShellToolType::LocalShell => {
            tools.push(OpenAiTool::LocalShell {});
        }
        ConfigShellToolType::StreamableShell => {
            tools.push(create_shell_tool());
        }
    }

    if config.plan_tool {
        tools.push(PLAN_TOOL.clone());
    }

    if let Some(apply_patch_tool_type) = &config.apply_patch_tool_type {
        match apply_patch_tool_type {
            ApplyPatchToolType::Freeform => {
                tools.push(create_apply_patch_freeform_tool());
            }
            ApplyPatchToolType::Function => {
                tools.push(create_apply_patch_json_tool());
            }
        }
    }

    if config.web_search_request {
        tools.push(OpenAiTool::WebSearch {});
    }

    // Include the view_image tool so the agent can attach images to context.
    if config.include_view_image_tool {
        tools.push(create_view_image_tool());
    }

    tools
}
```

### slide-rs/core/src/bash.rs
```rust
use tree_sitter::Parser;
use tree_sitter::Tree;
use tree_sitter_bash::LANGUAGE as BASH;

/// Parse the provided bash source using tree-sitter-bash, returning a Tree on
/// success or None if parsing failed.
pub fn try_parse_bash(bash_lc_arg: &str) -> Option<Tree> {
    let lang = BASH.into();
    let mut parser = Parser::new();
    #[expect(clippy::expect_used)]
    parser.set_language(&lang).expect("load bash grammar");
    let old_tree: Option<&Tree> = None;
    parser.parse(bash_lc_arg, old_tree)
}

/// Parse a script which may contain multiple simple commands joined only by
/// the safe logical/pipe/sequencing operators: `&&`, `||`, `;`, `|`.
///
/// Returns `Some(Vec<command_words>)` if every command is a plain word‑only
/// command and the parse tree does not contain disallowed constructs
/// (parentheses, redirections, substitutions, control flow, etc.). Otherwise
/// returns `None`.
pub fn try_parse_word_only_commands_sequence(tree: &Tree, src: &str) -> Option<Vec<Vec<String>>> {
    if tree.root_node().has_error() {
        return None;
    }

    // List of allowed (named) node kinds for a "word only commands sequence".
    // If we encounter a named node that is not in this list we reject.
    const ALLOWED_KINDS: &[&str] = &[
        // top level containers
        "program",
        "list",
        "pipeline",
        // commands & words
        "command",
        "command_name",
        "word",
        "string",
        "string_content",
        "raw_string",
        "number",
    ];
    // Allow only safe punctuation / operator tokens; anything else causes reject.
    const ALLOWED_PUNCT_TOKENS: &[&str] = &["&&", "||", ";", "|", "\"", "'"];

    let root = tree.root_node();
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut command_nodes = Vec::new();
    while let Some(node) = stack.pop() {
        let kind = node.kind();
        if node.is_named() {
            if !ALLOWED_KINDS.contains(&kind) {
                return None;
            }
            if kind == "command" {
                command_nodes.push(node);
            }
        } else {
            // Reject any punctuation / operator tokens that are not explicitly allowed.
            if kind.chars().any(|c| "&;|".contains(c)) && !ALLOWED_PUNCT_TOKENS.contains(&kind) {
                return None;
            }
            if !(ALLOWED_PUNCT_TOKENS.contains(&kind) || kind.trim().is_empty()) {
                // If it's a quote token or operator it's allowed above; we also allow whitespace tokens.
                // Any other punctuation like parentheses, braces, redirects, backticks, etc are rejected.
                return None;
            }
        }
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    let mut commands = Vec::new();
    for node in command_nodes {
        if let Some(words) = parse_plain_command_from_node(node, src) {
            commands.push(words);
        } else {
            return None;
        }
    }
    Some(commands)
}

fn parse_plain_command_from_node(cmd: tree_sitter::Node, src: &str) -> Option<Vec<String>> {
    if cmd.kind() != "command" {
        return None;
    }
    let mut words = Vec::new();
    let mut cursor = cmd.walk();
    for child in cmd.named_children(&mut cursor) {
        match child.kind() {
            "command_name" => {
                let word_node = child.named_child(0)?;
                if word_node.kind() != "word" {
                    return None;
                }
                words.push(word_node.utf8_text(src.as_bytes()).ok()?.to_owned());
            }
            "word" | "number" => {
                words.push(child.utf8_text(src.as_bytes()).ok()?.to_owned());
            }
            "string" => {
                if child.child_count() == 3
                    && child.child(0)?.kind() == "\""
                    && child.child(1)?.kind() == "string_content"
                    && child.child(2)?.kind() == "\""
                {
                    words.push(child.child(1)?.utf8_text(src.as_bytes()).ok()?.to_owned());
                } else {
                    return None;
                }
            }
            "raw_string" => {
                let raw_string = child.utf8_text(src.as_bytes()).ok()?;
                let stripped = raw_string
                    .strip_prefix('\'')
                    .and_then(|s| s.strip_suffix('\''));
                if let Some(s) = stripped {
                    words.push(s.to_owned());
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_seq(src: &str) -> Option<Vec<Vec<String>>> {
        let tree = try_parse_bash(src)?;
        try_parse_word_only_commands_sequence(&tree, src)
    }

    #[test]
    fn accepts_single_simple_command() {
        let cmds = parse_seq("ls -1").unwrap();
        assert_eq!(cmds, vec![vec!["ls".to_string(), "-1".to_string()]]);
    }

    #[test]
    fn accepts_multiple_commands_with_allowed_operators() {
        let src = "ls && pwd; echo 'hi there' | wc -l";
        let cmds = parse_seq(src).unwrap();
        let expected: Vec<Vec<String>> = vec![
            vec!["wc".to_string(), "-l".to_string()],
            vec!["echo".to_string(), "hi there".to_string()],
            vec!["pwd".to_string()],
            vec!["ls".to_string()],
        ];
        assert_eq!(cmds, expected);
    }

    #[test]
    fn extracts_double_and_single_quoted_strings() {
        let cmds = parse_seq("echo \"hello world\"").unwrap();
        assert_eq!(
            cmds,
            vec![vec!["echo".to_string(), "hello world".to_string()]]
        );

        let cmds2 = parse_seq("echo 'hi there'").unwrap();
        assert_eq!(
            cmds2,
            vec![vec!["echo".to_string(), "hi there".to_string()]]
        );
    }

    #[test]
    fn accepts_numbers_as_words() {
        let cmds = parse_seq("echo 123 456").unwrap();
        assert_eq!(
            cmds,
            vec![vec![
                "echo".to_string(),
                "123".to_string(),
                "456".to_string()
            ]]
        );
    }

    #[test]
    fn rejects_parentheses_and_subshells() {
        assert!(parse_seq("(ls)").is_none());
        assert!(parse_seq("ls || (pwd && echo hi)").is_none());
    }

    #[test]
    fn rejects_redirections_and_unsupported_operators() {
        assert!(parse_seq("ls > out.txt").is_none());
        assert!(parse_seq("echo hi & echo bye").is_none());
    }

    #[test]
    fn rejects_command_and_process_substitutions_and_expansions() {
        assert!(parse_seq("echo $(pwd)").is_none());
        assert!(parse_seq("echo `pwd`").is_none());
        assert!(parse_seq("echo $HOME").is_none());
        assert!(parse_seq("echo \"hi $USER\"").is_none());
    }

    #[test]
    fn rejects_variable_assignment_prefix() {
        assert!(parse_seq("FOO=bar ls").is_none());
    }

    #[test]
    fn rejects_trailing_operator_parse_error() {
        assert!(parse_seq("ls &&").is_none());
    }
}
```

### slide-rs/core/src/shell.rs
```rust
use serde::Deserialize;
use serde::Serialize;
use shlex;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct ZshShell {
    shell_path: String,
    zshrc_path: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct PowerShellConfig {
    exe: String, // Executable name or path, e.g. "pwsh" or "powershell.exe".
    bash_exe_fallback: Option<PathBuf>, // In case the model generates a bash command.
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum Shell {
    Zsh(ZshShell),
    PowerShell(PowerShellConfig),
    Unknown,
}

impl Shell {
    pub fn format_default_shell_invocation(&self, command: Vec<String>) -> Option<Vec<String>> {
        match self {
            Shell::Zsh(zsh) => {
                if !std::path::Path::new(&zsh.zshrc_path).exists() {
                    return None;
                }

                let mut result = vec![zsh.shell_path.clone()];
                result.push("-lc".to_string());

                let joined = strip_bash_lc(&command)
                    .or_else(|| shlex::try_join(command.iter().map(|s| s.as_str())).ok());

                if let Some(joined) = joined {
                    result.push(format!("source {} && ({joined})", zsh.zshrc_path));
                } else {
                    return None;
                }
                Some(result)
            }
            Shell::PowerShell(ps) => {
                // If model generated a bash command, prefer a detected bash fallback
                if let Some(script) = strip_bash_lc(&command) {
                    return match &ps.bash_exe_fallback {
                        Some(bash) => Some(vec![
                            bash.to_string_lossy().to_string(),
                            "-lc".to_string(),
                            script,
                        ]),

                        // No bash fallback → run the script under PowerShell.
                        None => Some(vec![
                            ps.exe.clone(),
                            "-NoProfile".to_string(),
                            "-Command".to_string(),
                            script,
                        ]),
                    };
                }

                // Not a bash command. If model did not generate a PowerShell command,
                // turn it into a PowerShell command.
                let first = command.first().map(String::as_str);
                if first != Some(ps.exe.as_str()) {
                    let joined = shlex::try_join(command.iter().map(|s| s.as_str())).ok();
                    return joined.map(|arg| {
                        vec![
                            ps.exe.clone(),
                            "-NoProfile".to_string(),
                            "-Command".to_string(),
                            arg,
                        ]
                    });
                }

                // Model generated a PowerShell command. Run it.
                Some(command)
            }
            Shell::Unknown => None,
        }
    }

    pub fn name(&self) -> Option<String> {
        match self {
            Shell::Zsh(zsh) => std::path::Path::new(&zsh.shell_path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string()),
            Shell::PowerShell(ps) => Some(ps.exe.clone()),
            Shell::Unknown => None,
        }
    }
}

fn strip_bash_lc(command: &Vec<String>) -> Option<String> {
    match command.as_slice() {
        // exactly three items
        [first, second, third]
            // first two must be "bash", "-lc"
            if first == "bash" && second == "-lc" =>
        {
            Some(third.clone())
        }
        _ => None,
    }
}

#[cfg(target_os = "macos")]
pub async fn default_user_shell() -> Shell {
    use tokio::process::Command;
    use whoami;

    let user = whoami::username();
    let home = format!("/Users/{user}");
    let output = Command::new("dscl")
        .args([".", "-read", &home, "UserShell"])
        .output()
        .await
        .ok();
    match output {
        Some(o) => {
            if !o.status.success() {
                return Shell::Unknown;
            }
            let stdout = String::from_utf8_lossy(&o.stdout);
            for line in stdout.lines() {
                if let Some(shell_path) = line.strip_prefix("UserShell: ")
                    && shell_path.ends_with("/zsh")
                {
                    return Shell::Zsh(ZshShell {
                        shell_path: shell_path.to_string(),
                        zshrc_path: format!("{home}/.zshrc"),
                    });
                }
            }

            Shell::Unknown
        }
        _ => Shell::Unknown,
    }
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
pub async fn default_user_shell() -> Shell {
    Shell::Unknown
}

#[cfg(target_os = "windows")]
pub async fn default_user_shell() -> Shell {
    use tokio::process::Command;

    // Prefer PowerShell 7+ (`pwsh`) if available, otherwise fall back to Windows PowerShell.
    let has_pwsh = Command::new("pwsh")
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-Command")
        .arg("$PSVersionTable.PSVersion.Major")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);
    let bash_exe = if Command::new("bash.exe")
        .arg("--version")
        .output()
        .await
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        which::which("bash.exe").ok()
    } else {
        None
    };

    if has_pwsh {
        Shell::PowerShell(PowerShellConfig {
            exe: "pwsh.exe".to_string(),
            bash_exe_fallback: bash_exe,
        })
    } else {
        Shell::PowerShell(PowerShellConfig {
            exe: "powershell.exe".to_string(),
            bash_exe_fallback: bash_exe,
        })
    }
}
```

### slide-rs/core/src/client.rs
```rust
use std::io::BufRead;
use std::path::Path;
use std::time::Duration;

use bytes::Bytes;
use slide_login::AuthManager;
use slide_login::AuthMode;
use eventsource_stream::Eventsource;
use futures::prelude::*;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::io::ReaderStream;
use tracing::debug;
use tracing::trace;
use tracing::warn;
use uuid::Uuid;

use crate::chat_completions::AggregateStreamExt;
use crate::chat_completions::stream_chat_completions;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::client_common::ResponsesApiRequest;
use crate::client_common::create_reasoning_param_for_request;
use crate::client_common::create_text_param_for_request;
use crate::config::Config;
use crate::error::SlideErr;
use crate::error::Result;
use crate::error::UsageLimitReachedError;
use crate::flags::SLIDE_RS_SSE_FIXTURE;
use crate::model_family::ModelFamily;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::WireApi;
use crate::openai_model_info::get_model_info;
use crate::openai_tools::create_tools_json_for_responses_api;
use crate::protocol::TokenUsage;
use crate::user_agent::get_slide_user_agent;
use crate::util::backoff;
use slide_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use slide_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use slide_protocol::models::ResponseItem;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: Error,
}

#[derive(Debug, Deserialize)]
struct Error {
    r#type: Option<String>,
    message: Option<String>,

    // Optional fields available on "usage_limit_reached" and "usage_not_included" errors
    plan_type: Option<String>,
    resets_in_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ModelClient {
    config: Arc<Config>,
    auth_manager: Option<Arc<AuthManager>>,
    client: reqwest::Client,
    provider: ModelProviderInfo,
    session_id: Uuid,
    effort: ReasoningEffortConfig,
    summary: ReasoningSummaryConfig,
}

impl ModelClient {
    pub fn new(
        config: Arc<Config>,
        auth_manager: Option<Arc<AuthManager>>,
        provider: ModelProviderInfo,
        effort: ReasoningEffortConfig,
        summary: ReasoningSummaryConfig,
        session_id: Uuid,
    ) -> Self {
        Self {
            config,
            auth_manager,
            client: reqwest::Client::new(),
            provider,
            session_id,
            effort,
            summary,
        }
    }

    pub fn get_model_context_window(&self) -> Option<u64> {
        self.config
            .model_context_window
            .or_else(|| get_model_info(&self.config.model_family).map(|info| info.context_window))
    }

    /// Dispatches to either the Responses or Chat implementation depending on
    /// the provider config.  Public callers always invoke `stream()` – the
    /// specialised helpers are private to avoid accidental misuse.
    pub async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream> {
        match self.provider.wire_api {
            WireApi::Responses => self.stream_responses(prompt).await,
            WireApi::Chat => {
                // Create the raw streaming connection first.
                let response_stream = stream_chat_completions(
                    prompt,
                    &self.config.model_family,
                    &self.client,
                    &self.provider,
                )
                .await?;

                // Wrap it with the aggregation adapter so callers see *only*
                // the final assistant message per turn (matching the
                // behaviour of the Responses API).
                let mut aggregated = if self.config.show_raw_agent_reasoning {
                    crate::chat_completions::AggregatedChatStream::streaming_mode(response_stream)
                } else {
                    response_stream.aggregate()
                };

                // Bridge the aggregated stream back into a standard
                // `ResponseStream` by forwarding events through a channel.
                let (tx, rx) = mpsc::channel::<Result<ResponseEvent>>(16);

                tokio::spawn(async move {
                    use futures::StreamExt;
                    while let Some(ev) = aggregated.next().await {
                        // Exit early if receiver hung up.
                        if tx.send(ev).await.is_err() {
                            break;
                        }
                    }
                });

                Ok(ResponseStream { rx_event: rx })
            }
        }
    }

    /// Implementation for the OpenAI *Responses* experimental API.
    async fn stream_responses(&self, prompt: &Prompt) -> Result<ResponseStream> {
        if let Some(path) = &*SLIDE_RS_SSE_FIXTURE {
            // short circuit for tests
            warn!(path, "Streaming from fixture");
            return stream_from_fixture(path, self.provider.clone()).await;
        }

        let auth_manager = self.auth_manager.clone();

        let auth_mode = auth_manager
            .as_ref()
            .and_then(|m| m.auth())
            .as_ref()
            .map(|a| a.mode);

        let store = prompt.store && auth_mode != Some(AuthMode::ChatGPT);

        let full_instructions = prompt.get_full_instructions(&self.config.model_family);
        let tools_json = create_tools_json_for_responses_api(&prompt.tools)?;
        let reasoning = create_reasoning_param_for_request(
            &self.config.model_family,
            self.effort,
            self.summary,
        );

        // Request encrypted COT if we are not storing responses,
        // otherwise reasoning items will be referenced by ID
        let include: Vec<String> = if !store && reasoning.is_some() {
            vec!["reasoning.encrypted_content".to_string()]
        } else {
            vec![]
        };

        let input_with_instructions = prompt.get_formatted_input();

        // Only include `text.verbosity` for GPT-5 family models
        let text = if self.config.model_family.family == "gpt-5" {
            create_text_param_for_request(self.config.model_verbosity)
        } else {
            if self.config.model_verbosity.is_some() {
                warn!(
                    "model_verbosity is set but ignored for non-gpt-5 model family: {}",
                    self.config.model_family.family
                );
            }
            None
        };

        let payload = ResponsesApiRequest {
            model: &self.config.model,
            instructions: &full_instructions,
            input: &input_with_instructions,
            tools: &tools_json,
            tool_choice: "auto",
            parallel_tool_calls: false,
            reasoning,
            store,
            stream: true,
            include,
            prompt_cache_key: Some(self.session_id.to_string()),
            text,
        };

        let mut attempt = 0;
        let max_retries = self.provider.request_max_retries();

        loop {
            attempt += 1;

            // Always fetch the latest auth in case a prior attempt refreshed the token.
            let auth = auth_manager.as_ref().and_then(|m| m.auth());

            trace!(
                "POST to {}: {}",
                self.provider.get_full_url(&auth),
                serde_json::to_string(&payload)?
            );

            let mut req_builder = self
                .provider
                .create_request_builder(&self.client, &auth)
                .await?;

            req_builder = req_builder
                .header("OpenAI-Beta", "responses=experimental")
                .header("session_id", self.session_id.to_string())
                .header(reqwest::header::ACCEPT, "text/event-stream")
                .json(&payload);

            if let Some(auth) = auth.as_ref()
                && auth.mode == AuthMode::ChatGPT
                && let Some(account_id) = auth.get_account_id()
            {
                req_builder = req_builder.header("chatgpt-account-id", account_id);
            }

            let originator = &self.config.responses_originator_header;
            req_builder = req_builder.header("originator", originator);
            req_builder = req_builder.header("User-Agent", get_slide_user_agent(Some(originator)));

            let res = req_builder.send().await;
            if let Ok(resp) = &res {
                trace!(
                    "Response status: {}, request-id: {}",
                    resp.status(),
                    resp.headers()
                        .get("x-request-id")
                        .map(|v| v.to_str().unwrap_or_default())
                        .unwrap_or_default()
                );
            }

            match res {
                Ok(resp) if resp.status().is_success() => {
                    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

                    // spawn task to process SSE
                    let stream = resp.bytes_stream().map_err(SlideErr::Reqwest);
                    tokio::spawn(process_sse(
                        stream,
                        tx_event,
                        self.provider.stream_idle_timeout(),
                    ));

                    return Ok(ResponseStream { rx_event });
                }
                Ok(res) => {
                    let status = res.status();

                    // Pull out Retry‑After header if present.
                    let retry_after_secs = res
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok());

                    if status == StatusCode::UNAUTHORIZED
                        && let Some(manager) = auth_manager.as_ref()
                        && manager.auth().is_some()
                    {
                        let _ = manager.refresh_token().await;
                    }

                    if !(status == StatusCode::TOO_MANY_REQUESTS
                        || status == StatusCode::UNAUTHORIZED
                        || status.is_server_error())
                    {
                        // Surface the error body to callers. Use `unwrap_or_default` per Clippy.
                        let body = res.text().await.unwrap_or_default();
                        return Err(SlideErr::UnexpectedStatus(status, body));
                    }

                    if status == StatusCode::TOO_MANY_REQUESTS {
                        let body = res.json::<ErrorResponse>().await.ok();
                        if let Some(ErrorResponse { error }) = body {
                            if error.r#type.as_deref() == Some("usage_limit_reached") {
                                // Prefer the plan_type provided in the error message if present
                                let plan_type = error
                                    .plan_type
                                    .or_else(|| auth.and_then(|a| a.get_plan_type()));
                                let resets_in_seconds = error.resets_in_seconds;
                                return Err(SlideErr::UsageLimitReached(UsageLimitReachedError {
                                    plan_type,
                                    resets_in_seconds,
                                }));
                            } else if error.r#type.as_deref() == Some("usage_not_included") {
                                return Err(SlideErr::UsageNotIncluded);
                            }
                        }
                    }

                    if attempt > max_retries {
                        if status == StatusCode::INTERNAL_SERVER_ERROR {
                            return Err(SlideErr::InternalServerError);
                        }

                        return Err(SlideErr::RetryLimit(status));
                    }

                    let delay = retry_after_secs
                        .map(|s| Duration::from_millis(s * 1_000))
                        .unwrap_or_else(|| backoff(attempt));
                    tokio::time::sleep(delay).await;
                }
                Err(e) => {
                    if attempt > max_retries {
                        return Err(e.into());
                    }
                    let delay = backoff(attempt);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    pub fn get_provider(&self) -> ModelProviderInfo {
        self.provider.clone()
    }

    /// Returns the currently configured model slug.
    pub fn get_model(&self) -> String {
        self.config.model.clone()
    }

    /// Returns the currently configured model family.
    pub fn get_model_family(&self) -> ModelFamily {
        self.config.model_family.clone()
    }

    /// Returns the current reasoning effort setting.
    pub fn get_reasoning_effort(&self) -> ReasoningEffortConfig {
        self.effort
    }

    /// Returns the current reasoning summary setting.
    pub fn get_reasoning_summary(&self) -> ReasoningSummaryConfig {
        self.summary
    }

    pub fn get_auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }
}

async fn process_sse<S>(
    stream: S,
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    idle_timeout: Duration,
) where
    S: Stream<Item = Result<Bytes>> + Unpin,
{
    let mut stream = stream.eventsource();

    // If the stream stays completely silent for an extended period treat it as disconnected.
    let mut response_error: Option<SlideErr> = None;

    loop {
        let sse = match timeout(idle_timeout, stream.next()).await {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                debug!("SSE Error: {e:#}");
                let event = SlideErr::Stream(e.to_string(), None);
                let _ = tx_event.send(Err(event)).await;
                return;
            }
            Ok(None) => {
                let _ = tx_event
                    .send(Err(response_error.unwrap_or(SlideErr::Stream(
                        "stream closed unexpectedly".into(),
                        None,
                    ))))
                    .await;
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(SlideErr::Stream(
                        "idle timeout waiting for SSE".into(),
                        None,
                    )))
                    .await;
                return;
            }
        };

        let raw = sse.data.clone();
        trace!("SSE event: {}", raw);

        // For slide generation, we primarily care about streaming text deltas
        let event = ResponseEvent::OutputTextDelta(raw);
        if tx_event.send(Ok(event)).await.is_err() {
            return;
        }
    }
}

/// used in tests to stream from a text SSE file
async fn stream_from_fixture(
    path: impl AsRef<Path>,
    provider: ModelProviderInfo,
) -> Result<ResponseStream> {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);
    let f = std::fs::File::open(path.as_ref())?;
    let lines = std::io::BufReader::new(f).lines();

    // insert \n\n after each line for proper SSE parsing
    let mut content = String::new();
    for line in lines {
        content.push_str(&line?);
        content.push_str("\n\n");
    }

    let rdr = std::io::Cursor::new(content);
    let stream = ReaderStream::new(rdr).map_err(SlideErr::Io);
    tokio::spawn(process_sse(
        stream,
        tx_event,
        provider.stream_idle_timeout(),
    ));
    Ok(ResponseStream { rx_event })
}
```

### slide-rs/core/src/client_common.rs
```rust
use crate::config_types::Verbosity as VerbosityConfig;
use crate::error::Result;
use crate::model_family::ModelFamily;
use crate::openai_tools::OpenAiTool;
use crate::protocol::TokenUsage;
use slide_apply_patch::APPLY_PATCH_TOOL_INSTRUCTIONS;
use slide_protocol::config_types::ReasoningEffort as ReasoningEffortConfig;
use slide_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use slide_protocol::models::ContentItem;
use slide_protocol::models::ResponseItem;
use futures::Stream;
use serde::Serialize;
use std::borrow::Cow;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;

/// The `instructions` field in the payload sent to a model should always start
/// with this content.
const BASE_INSTRUCTIONS: &str = include_str!("../prompt.md");

/// wraps user instructions message in a tag for the model to parse more easily.
const USER_INSTRUCTIONS_START: &str = "<user_instructions>\n\n";
const USER_INSTRUCTIONS_END: &str = "\n\n</user_instructions>";

/// API request payload for a single model turn
#[derive(Default, Debug, Clone)]
pub struct Prompt {
    /// Conversation context input items.
    pub input: Vec<ResponseItem>,

    /// Whether to store response on server side (disable_response_storage = !store).
    pub store: bool,

    /// Tools available to the model, including additional tools sourced from
    /// external servers.
    pub tools: Vec<OpenAiTool>,

    /// Optional override for the built-in BASE_INSTRUCTIONS.
    pub base_instructions_override: Option<String>,
}

impl Prompt {
    pub(crate) fn get_full_instructions(&self, model: &ModelFamily) -> Cow<'_, str> {
        let base = self
            .base_instructions_override
            .as_deref()
            .unwrap_or(BASE_INSTRUCTIONS);
        let mut sections: Vec<&str> = vec![base];

        // When there are no custom instructions, add apply_patch_tool_instructions if either:
        // - the model needs special instructions (4.1), or
        // - there is no apply_patch tool present
        let is_apply_patch_tool_present = self.tools.iter().any(|tool| match tool {
            OpenAiTool::Function(f) => f.name == "apply_patch",
            OpenAiTool::Freeform(f) => f.name == "apply_patch",
            _ => false,
        });
        if self.base_instructions_override.is_none()
            && (model.needs_special_apply_patch_instructions || !is_apply_patch_tool_present)
        {
            sections.push(APPLY_PATCH_TOOL_INSTRUCTIONS);
        }
        Cow::Owned(sections.join("\n"))
    }

    pub(crate) fn get_formatted_input(&self) -> Vec<ResponseItem> {
        self.input.clone()
    }

    /// Creates a formatted user instructions message from a string
    pub(crate) fn format_user_instructions_message(ui: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: format!("{USER_INSTRUCTIONS_START}{ui}{USER_INSTRUCTIONS_END}"),
            }],
        }
    }
}

#[derive(Debug)]
pub enum ResponseEvent {
    Created,
    OutputItemDone(ResponseItem),
    Completed {
        response_id: String,
        token_usage: Option<TokenUsage>,
    },
    OutputTextDelta(String),
    ReasoningSummaryDelta(String),
    ReasoningContentDelta(String),
    ReasoningSummaryPartAdded,
    WebSearchCallBegin {
        call_id: String,
    },
}

#[derive(Debug, Serialize)]
pub(crate) struct Reasoning {
    pub(crate) effort: ReasoningEffortConfig,
    pub(crate) summary: ReasoningSummaryConfig,
}

/// Controls under the `text` field in the Responses API for GPT-5.
#[derive(Debug, Serialize, Default, Clone, Copy)]
pub(crate) struct TextControls {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) verbosity: Option<OpenAiVerbosity>,
}

#[derive(Debug, Serialize, Default, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OpenAiVerbosity {
    Low,
    #[default]
    Medium,
    High,
}

impl From<VerbosityConfig> for OpenAiVerbosity {
    fn from(v: VerbosityConfig) -> Self {
        match v {
            VerbosityConfig::Low => OpenAiVerbosity::Low,
            VerbosityConfig::Medium => OpenAiVerbosity::Medium,
            VerbosityConfig::High => OpenAiVerbosity::High,
        }
    }
}

/// Request object that is serialized as JSON and POST'ed when using the
/// Responses API.
#[derive(Debug, Serialize)]
pub(crate) struct ResponsesApiRequest<'a> {
    pub(crate) model: &'a str,
    pub(crate) instructions: &'a str,
    // TODO: ResponseItem::Other should not be serialized. Currently,
    // we code defensively to avoid this case, but perhaps we should use a
    // separate enum for serialization.
    pub(crate) input: &'a Vec<ResponseItem>,
    pub(crate) tools: &'a [serde_json::Value],
    pub(crate) tool_choice: &'static str,
    pub(crate) parallel_tool_calls: bool,
    pub(crate) reasoning: Option<Reasoning>,
    /// true when using the Responses API.
    pub(crate) store: bool,
    pub(crate) stream: bool,
    pub(crate) include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<TextControls>,
}

pub(crate) fn create_reasoning_param_for_request(
    model_family: &ModelFamily,
    effort: ReasoningEffortConfig,
    summary: ReasoningSummaryConfig,
) -> Option<Reasoning> {
    if model_family.supports_reasoning_summaries {
        Some(Reasoning { effort, summary })
    } else {
        None
    }
}

pub(crate) fn create_text_param_for_request(
    verbosity: Option<VerbosityConfig>,
) -> Option<TextControls> {
    verbosity.map(|v| TextControls {
        verbosity: Some(v.into()),
    })
}

pub(crate) struct ResponseStream {
    pub(crate) rx_event: mpsc::Receiver<Result<ResponseEvent>>,
}

impl Stream for ResponseStream {
    type Item = Result<ResponseEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx_event.poll_recv(cx)
    }
}

#[cfg(test)]
mod tests {
    use crate::model_family::find_family_for_model;

    use super::*;

    #[test]
    fn get_full_instructions_no_user_content() {
        let prompt = Prompt {
            ..Default::default()
        };
        let expected = format!("{BASE_INSTRUCTIONS}\n{APPLY_PATCH_TOOL_INSTRUCTIONS}");
        let model_family = find_family_for_model("gpt-4.1").expect("known model slug");
        let full = prompt.get_full_instructions(&model_family);
        assert_eq!(full, expected);
    }

    #[test]
    fn serializes_text_verbosity_when_set() {
        let input: Vec<ResponseItem> = vec![];
        let tools: Vec<serde_json::Value> = vec![];
        let req = ResponsesApiRequest {
            model: "gpt-5",
            instructions: "i",
            input: &input,
            tools: &tools,
            tool_choice: "auto",
            parallel_tool_calls: false,
            reasoning: None,
            store: true,
            stream: true,
            include: vec![],
            prompt_cache_key: None,
            text: Some(TextControls {
                verbosity: Some(OpenAiVerbosity::Low),
            }),
        };

        let v = serde_json::to_value(&req).expect("json");
        assert_eq!(
            v.get("text")
                .and_then(|t| t.get("verbosity"))
                .and_then(|s| s.as_str()),
            Some("low")
        );
    }

    #[test]
    fn omits_text_when_not_set() {
        let input: Vec<ResponseItem> = vec![];
        let tools: Vec<serde_json::Value> = vec![];
        let req = ResponsesApiRequest {
            model: "gpt-5",
            instructions: "i",
            input: &input,
            tools: &tools,
            tool_choice: "auto",
            parallel_tool_calls: false,
            reasoning: None,
            store: true,
            stream: true,
            include: vec![],
            prompt_cache_key: None,
            text: None,
        };

        let v = serde_json::to_value(&req).expect("json");
        assert!(v.get("text").is_none());
    }
}
```

#### slide-rs/core/src/error.rs
```rust
use reqwest::StatusCode;
use serde_json;
use std::io;
use std::time::Duration;
use thiserror::Error;
use tokio::task::JoinError;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, SlideErr>;

#[derive(Error, Debug)]
pub enum SandboxErr {
    /// Error from sandbox execution
    #[error("sandbox denied exec error, exit code: {0}, stdout: {1}, stderr: {2}")]
    Denied(i32, String, String),

    /// Error from linux seccomp filter setup
    #[cfg(target_os = "linux")]
    #[error("seccomp setup error")]
    SeccompInstall(#[from] seccompiler::Error),

    /// Error from linux seccomp backend
    #[cfg(target_os = "linux")]
    #[error("seccomp backend error")]
    SeccompBackend(#[from] seccompiler::BackendError),

    /// Command timed out
    #[error("command timed out")]
    Timeout,

    /// Command was killed by a signal
    #[error("command was killed by a signal")]
    Signal(i32),

    /// Error from linux landlock
    #[error("Landlock was not able to fully enforce all sandbox rules")]
    LandlockRestrict,
}

#[derive(Error, Debug)]
pub enum SlideErr {
    /// Returned by ResponsesClient when the SSE stream disconnects or errors out **after** the HTTP
    /// handshake has succeeded but **before** it finished emitting `response.completed`.
    ///
    /// The Session loop treats this as a transient error and will automatically retry the turn.
    ///
    /// Optionally includes the requested delay before retrying the turn.
    #[error("stream disconnected before completion: {0}")]
    Stream(String, Option<Duration>),

    #[error("no conversation with id: {0}")]
    ConversationNotFound(Uuid),

    #[error("session configured event was not the first event in the stream")]
    SessionConfiguredNotFirstEvent,

    /// Returned by run_command_stream when the spawned child process timed out (10s).
    #[error("timeout waiting for child process to exit")]
    Timeout,

    /// Returned by run_command_stream when the child could not be spawned (its stdout/stderr pipes
    /// could not be captured). Analogous to the previous `SlideError::Spawn` variant.
    #[error("spawn failed: child stdout/stderr not captured")]
    Spawn,

    /// Returned by run_command_stream when the user pressed Ctrl‑C (SIGINT). Session uses this to
    /// surface a polite FunctionCallOutput back to the model instead of crashing the CLI.
    #[error("interrupted (Ctrl-C)")]
    Interrupted,

    /// Unexpected HTTP status code.
    #[error("unexpected status {0}: {1}")]
    UnexpectedStatus(StatusCode, String),

    #[error("{0}")]
    UsageLimitReached(UsageLimitReachedError),

    #[error(
        "To use Slide with your ChatGPT plan, upgrade to Plus: https://openai.com/chatgpt/pricing."
    )]
    UsageNotIncluded,

    #[error("We're currently experiencing high demand, which may cause temporary errors.")]
    InternalServerError,

    /// Retry limit exceeded.
    #[error("exceeded retry limit, last status: {0}")]
    RetryLimit(StatusCode),

    /// Agent loop died unexpectedly
    #[error("internal error; agent loop died unexpectedly")]
    InternalAgentDied,

    /// Sandbox error
    #[error("sandbox error: {0}")]
    Sandbox(#[from] SandboxErr),

    #[error("slide-linux-sandbox was required but not provided")]
    LandlockSandboxExecutableNotProvided,

    // -----------------------------------------------------------------
    // Automatic conversions for common external error types
    // -----------------------------------------------------------------
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[cfg(target_os = "linux")]
    #[error(transparent)]
    LandlockRuleset(#[from] landlock::RulesetError),

    #[cfg(target_os = "linux")]
    #[error(transparent)]
    LandlockPathFd(#[from] landlock::PathFdError),

    #[error(transparent)]
    TokioJoin(#[from] JoinError),

    #[error("{0}")]
    EnvVar(EnvVarError),
}

#[derive(Debug)]
pub struct UsageLimitReachedError {
    pub plan_type: Option<String>,
    pub resets_in_seconds: Option<u64>,
}

impl std::fmt::Display for UsageLimitReachedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Base message differs slightly for legacy ChatGPT Plus plan users.
        if let Some(plan_type) = &self.plan_type
            && plan_type == "plus"
        {
            write!(
                f,
                "You've hit your usage limit. Upgrade to Pro (https://openai.com/chatgpt/pricing) or try again"
            )?;
            if let Some(secs) = self.resets_in_seconds {
                let reset_duration = format_reset_duration(secs);
                write!(f, " in {reset_duration}.")?;
            } else {
                write!(f, " later.")?;
            }
        } else {
            write!(f, "You've hit your usage limit.")?;

            if let Some(secs) = self.resets_in_seconds {
                let reset_duration = format_reset_duration(secs);
                write!(f, " Try again in {reset_duration}.")?;
            } else {
                write!(f, " Try again later.")?;
            }
        }

        Ok(())
    }
}

fn format_reset_duration(total_secs: u64) -> String {
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3_600;
    let minutes = (total_secs % 3_600) / 60;

    let mut parts: Vec<String> = Vec::new();
    if days > 0 {
        let unit = if days == 1 { "day" } else { "days" };
        parts.push(format!("{days} {unit}"));
    }
    if hours > 0 {
        let unit = if hours == 1 { "hour" } else { "hours" };
        parts.push(format!("{hours} {unit}"));
    }
    if minutes > 0 {
        let unit = if minutes == 1 { "minute" } else { "minutes" };
        parts.push(format!("{minutes} {unit}"));
    }

    if parts.is_empty() {
        return "less than a minute".to_string();
    }

    match parts.len() {
        1 => parts[0].clone(),
        2 => format!("{} {}", parts[0], parts[1]),
        _ => format!("{} {} {}", parts[0], parts[1], parts[2]),
    }
}

#[derive(Debug)]
pub struct EnvVarError {
    /// Name of the environment variable that is missing.
    pub var: String,

    /// Optional instructions to help the user get a valid value for the
    /// variable and set it.
    pub instructions: Option<String>,
}

impl std::fmt::Display for EnvVarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Missing environment variable: `{}`.", self.var)?;
        if let Some(instructions) = &self.instructions {
            write!(f, " {instructions}")?;
        }
        Ok(())
    }
}

impl SlideErr {
    /// Minimal shim so that existing `e.downcast_ref::<SlideErr>()` checks continue to compile
    /// after replacing `anyhow::Error` in the return signature. This mirrors the behavior of
    /// `anyhow::Error::downcast_ref` but works directly on our concrete enum.
    pub fn downcast_ref<T: std::any::Any>(&self) -> Option<&T> {
        (self as &dyn std::any::Any).downcast_ref::<T>()
    }
}

pub fn get_error_message_ui(e: &SlideErr) -> String {
    match e {
        SlideErr::Sandbox(SandboxErr::Denied(_, _, stderr)) => stderr.to_string(),
        // Timeouts are not sandbox errors from a UX perspective; present them plainly
        SlideErr::Sandbox(SandboxErr::Timeout) => "error: command timed out".to_string(),
        _ => e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_limit_reached_error_formats_plus_plan() {
        let err = UsageLimitReachedError {
            plan_type: Some("plus".to_string()),
            resets_in_seconds: None,
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Upgrade to Pro (https://openai.com/chatgpt/pricing) or try again later."
        );
    }

    #[test]
    fn usage_limit_reached_error_formats_default_when_none() {
        let err = UsageLimitReachedError {
            plan_type: None,
            resets_in_seconds: None,
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Try again later."
        );
    }

    #[test]
    fn usage_limit_reached_error_formats_default_for_other_plans() {
        let err = UsageLimitReachedError {
            plan_type: Some("pro".to_string()),
            resets_in_seconds: None,
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Try again later."
        );
    }

    #[test]
    fn usage_limit_reached_includes_minutes_when_available() {
        let err = UsageLimitReachedError {
            plan_type: None,
            resets_in_seconds: Some(5 * 60),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Try again in 5 minutes."
        );
    }

    #[test]
    fn usage_limit_reached_includes_hours_and_minutes() {
        let err = UsageLimitReachedError {
            plan_type: Some("plus".to_string()),
            resets_in_seconds: Some(3 * 3600 + 32 * 60),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Upgrade to Pro (https://openai.com/chatgpt/pricing) or try again in 3 hours 32 minutes."
        );
    }

    #[test]
    fn usage_limit_reached_includes_days_hours_minutes() {
        let err = UsageLimitReachedError {
            plan_type: None,
            resets_in_seconds: Some(2 * 86_400 + 3 * 3600 + 5 * 60),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Try again in 2 days 3 hours 5 minutes."
        );
    }

    #[test]
    fn usage_limit_reached_less_than_minute() {
        let err = UsageLimitReachedError {
            plan_type: None,
            resets_in_seconds: Some(30),
        };
        assert_eq!(
            err.to_string(),
            "You've hit your usage limit. Try again in less than a minute."
        );
    }
}
```

#### slide-rs/core/src/lib.rs
```rust
//! Root of the `slide-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user-visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod apply_patch;
mod bash;
mod chat_completions;
mod client;
mod client_common;
pub mod slide;
mod slide_conversation;
pub use slide_conversation::SlideConversation;
pub mod config;
pub mod config_profile;
pub mod config_types;
mod conversation_history;
pub mod custom_prompts;
mod environment_context;
pub mod error;
pub mod exec;
mod exec_command;
pub mod exec_env;
mod flags;
pub mod git_info;
mod is_safe_command;
pub mod landlock;
mod mcp_connection_manager;
mod mcp_tool_call;
mod message_history;
mod model_provider_info;
pub mod parse_command;
pub use model_provider_info::BUILT_IN_OSS_MODEL_PROVIDER_ID;
pub use model_provider_info::ModelProviderInfo;
pub use model_provider_info::WireApi;
pub use model_provider_info::built_in_model_providers;
pub use model_provider_info::create_oss_provider_with_base_url;
mod conversation_manager;
pub use conversation_manager::ConversationManager;
pub use conversation_manager::NewConversation;
pub mod model_family;
mod openai_model_info;
mod openai_tools;
pub mod plan_tool;
pub mod project_doc;
mod rollout;
pub(crate) mod safety;
pub mod seatbelt;
pub mod shell;
pub mod spawn;
pub mod terminal;
mod tool_apply_patch;
pub mod turn_diff_tracker;
pub mod user_agent;
mod user_notification;
pub mod util;
pub use apply_patch::SLIDE_APPLY_PATCH_ARG1;
pub use safety::get_platform_sandbox;
// Re-export the protocol types from the standalone `slide-protocol` crate so existing
// `slide_core::protocol::...` references continue to work across the workspace.
pub use slide_protocol::protocol;
// Re-export protocol config enums to ensure call sites can use the same types
// as those in the protocol crate when constructing protocol messages.
pub use slide_protocol::config_types as protocol_config_types;
```

#### slide-rs/core/src/config.rs
```rust
use crate::config_profile::ConfigProfile;
use crate::config_types::History;
use crate::config_types::McpServerConfig;
use crate::config_types::SandboxWorkspaceWrite;
use crate::config_types::ShellEnvironmentPolicy;
use crate::config_types::ShellEnvironmentPolicyToml;
use crate::config_types::Tui;
use crate::config_types::UriBasedFileOpener;
use crate::config_types::Verbosity;
use crate::git_info::resolve_root_git_project_for_trust;
use crate::model_family::ModelFamily;
use crate::model_family::find_family_for_model;
use crate::model_provider_info::ModelProviderInfo;
use crate::model_provider_info::built_in_model_providers;
use crate::openai_model_info::get_model_info;
use crate::protocol::AskForApproval;
use crate::protocol::SandboxPolicy;
use slide_login::AuthMode;
use slide_protocol::config_types::ReasoningEffort;
use slide_protocol::config_types::ReasoningSummary;
use slide_protocol::config_types::SandboxMode;
use dirs::home_dir;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use toml::Value as TomlValue;
use toml_edit::DocumentMut;

const OPENAI_DEFAULT_MODEL: &str = "gpt-5";

/// Maximum number of bytes of the documentation that will be embedded. Larger
/// files are *silently truncated* to this size so we do not take up too much of
/// the context window.
pub(crate) const PROJECT_DOC_MAX_BYTES: usize = 32 * 1024; // 32 KiB

const CONFIG_TOML_FILE: &str = "config.toml";

const DEFAULT_RESPONSES_ORIGINATOR_HEADER: &str = "slide_cli_rs";

/// Application configuration loaded from disk and merged with overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// Optional override of model selection.
    pub model: String,

    pub model_family: ModelFamily,

    /// Size of the context window for the model, in tokens.
    pub model_context_window: Option<u64>,

    /// Maximum number of output tokens.
    pub model_max_output_tokens: Option<u64>,

    /// Key into the model_providers map that specifies which provider to use.
    pub model_provider_id: String,

    /// Info needed to make an API request to the model.
    pub model_provider: ModelProviderInfo,

    /// Approval policy for executing commands.
    pub approval_policy: AskForApproval,

    pub sandbox_policy: SandboxPolicy,

    pub shell_environment_policy: ShellEnvironmentPolicy,

    /// When `true`, `AgentReasoning` events emitted by the backend will be
    /// suppressed from the frontend output. This can reduce visual noise when
    /// users are only interested in the final agent responses.
    pub hide_agent_reasoning: bool,

    /// When set to `true`, `AgentReasoningRawContentEvent` events will be shown in the UI/output.
    /// Defaults to `false`.
    pub show_raw_agent_reasoning: bool,

    /// Disable server-side response storage (sends the full conversation
    /// context with every request). Currently necessary for OpenAI customers
    /// who have opted into Zero Data Retention (ZDR).
    pub disable_response_storage: bool,

    /// User-provided instructions from AGENTS.md.
    pub user_instructions: Option<String>,

    /// Base instructions override.
    pub base_instructions: Option<String>,

    /// Optional external notifier command. When set, Slide will spawn this
    /// program after each completed *turn* (i.e. when the agent finishes
    /// processing a user submission). The value must be the full command
    /// broken into argv tokens **without** the trailing JSON argument - Slide
    /// appends one extra argument containing a JSON payload describing the
    /// event.
    ///
    /// Example `~/.slide/config.toml` snippet:
    ///
    /// ```toml
    /// notify = ["notify-send", "Slide"]
    /// ```
    ///
    /// which will be invoked as:
    ///
    /// ```shell
    /// notify-send Slide '{"type":"agent-turn-complete","turn-id":"12345"}'
    /// ```
    ///
    /// If unset the feature is disabled.
    pub notify: Option<Vec<String>>,

    /// The directory that should be treated as the current working directory
    /// for the session. All relative paths inside the business-logic layer are
    /// resolved against this path.
    pub cwd: PathBuf,

    /// Definition for MCP servers that Slide can reach out to for tool calls.
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Combined provider map (defaults merged with user-defined overrides).
    pub model_providers: HashMap<String, ModelProviderInfo>,

    /// Maximum number of bytes to include from an AGENTS.md project doc file.
    pub project_doc_max_bytes: usize,

    /// Directory containing all Slide state (defaults to `~/.slide` but can be
    /// overridden by the `SLIDE_HOME` environment variable).
    pub slide_home: PathBuf,

    /// Settings that govern if and what will be written to `~/.slide/history.jsonl`.
    pub history: History,

    /// Optional URI-based file opener. If set, citations to files in the model
    /// output will be hyperlinked using the specified URI scheme.
    pub file_opener: UriBasedFileOpener,

    /// Collection of settings that are specific to the TUI.
    pub tui: Tui,

    /// Path to the `slide-linux-sandbox` executable. This must be set if
    /// [`crate::exec::SandboxType::LinuxSeccomp`] is used. Note that this
    /// cannot be set in the config file: it must be set in code via
    /// [`ConfigOverrides`].
    ///
    /// When this program is invoked, arg0 will be set to `slide-linux-sandbox`.
    pub slide_linux_sandbox_exe: Option<PathBuf>,

    /// Value to use for `reasoning.effort` when making a request using the
    /// Responses API.
    pub model_reasoning_effort: ReasoningEffort,

    /// If not "none", the value to use for `reasoning.summary` when making a
    /// request using the Responses API.
    pub model_reasoning_summary: ReasoningSummary,

    /// Optional verbosity control for GPT-5 models (Responses API `text.verbosity`).
    pub model_verbosity: Option<Verbosity>,

    /// Base URL for requests to ChatGPT (as opposed to the OpenAI API).
    pub chatgpt_base_url: String,

    /// Experimental rollout resume path (absolute path to .jsonl; undocumented).
    pub experimental_resume: Option<PathBuf>,

    /// Include an experimental plan tool that the model can use to update its current plan and status of each step.
    pub include_plan_tool: bool,

    /// Include the `apply_patch` tool for models that benefit from invoking
    /// file edits as a structured tool call. When unset, this falls back to the
    /// model family's default preference.
    pub include_apply_patch_tool: bool,

    pub tools_web_search_request: bool,

    /// The value for the `originator` header included with Responses API requests.
    pub responses_originator_header: String,

    /// If set to `true`, the API key will be signed with the `originator` header.
    pub preferred_auth_method: AuthMode,

    pub use_experimental_streamable_shell_tool: bool,

    /// Include the `view_image` tool that lets the agent attach a local image path to context.
    pub include_view_image_tool: bool,
    /// When true, disables burst-paste detection for typed input entirely.
    /// All characters are inserted as they are received, and no buffering
    /// or placeholder replacement will occur for fast keypress bursts.
    pub disable_paste_burst: bool,
}

impl Config {
    /// Load configuration with *generic* CLI overrides (`-c key=value`) applied
    /// **in between** the values parsed from `config.toml` and the
    /// strongly-typed overrides specified via [`ConfigOverrides`].
    ///
    /// The precedence order is therefore: `config.toml` < `-c` overrides <
    /// `ConfigOverrides`.
    pub fn load_with_cli_overrides(
        cli_overrides: Vec<(String, TomlValue)>,
        overrides: ConfigOverrides,
    ) -> std::io::Result<Self> {
        // Resolve the directory that stores Slide state (e.g. ~/.slide or the
        // value of $SLIDE_HOME) so we can embed it into the resulting
        // `Config` instance.
        let slide_home = find_slide_home()?;

        // Step 1: parse `config.toml` into a generic JSON value.
        let mut root_value = load_config_as_toml(&slide_home)?;

        // Step 2: apply the `-c` overrides.
        for (path, value) in cli_overrides.into_iter() {
            apply_toml_override(&mut root_value, &path, value);
        }

        // Step 3: deserialize into `ConfigToml` so that Serde can enforce the
        // correct types.
        let cfg: ConfigToml = root_value.try_into().map_err(|e| {
            tracing::error!("Failed to deserialize overridden config: {e}");
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;

        // Step 4: merge with the strongly-typed overrides.
        Self::load_from_base_config_with_overrides(cfg, overrides, slide_home)
    }
}

pub fn load_config_as_toml_with_cli_overrides(
    slide_home: &Path,
    cli_overrides: Vec<(String, TomlValue)>,
) -> std::io::Result<ConfigToml> {
    let mut root_value = load_config_as_toml(slide_home)?;

    for (path, value) in cli_overrides.into_iter() {
        apply_toml_override(&mut root_value, &path, value);
    }

    let cfg: ConfigToml = root_value.try_into().map_err(|e| {
        tracing::error!("Failed to deserialize overridden config: {e}");
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })?;

    Ok(cfg)
}

/// Read `SLIDE_HOME/config.toml` and return it as a generic TOML value. Returns
/// an empty TOML table when the file does not exist.
pub fn load_config_as_toml(slide_home: &Path) -> std::io::Result<TomlValue> {
    let config_path = slide_home.join(CONFIG_TOML_FILE);
    match std::fs::read_to_string(&config_path) {
        Ok(contents) => match toml::from_str::<TomlValue>(&contents) {
            Ok(val) => Ok(val),
            Err(e) => {
                tracing::error!("Failed to parse config.toml: {e}");
                Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!("config.toml not found, using defaults");
            Ok(TomlValue::Table(Default::default()))
        }
        Err(e) => {
            tracing::error!("Failed to read config.toml: {e}");
            Err(e)
        }
    }
}

/// Patch `SLIDE_HOME/config.toml` project state.
/// Use with caution.
pub fn set_project_trusted(slide_home: &Path, project_path: &Path) -> anyhow::Result<()> {
    let config_path = slide_home.join(CONFIG_TOML_FILE);
    // Parse existing config if present; otherwise start a new document.
    let mut doc = match std::fs::read_to_string(config_path.clone()) {
        Ok(s) => s.parse::<DocumentMut>()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DocumentMut::new(),
        Err(e) => return Err(e.into()),
    };

    // Ensure we render a human-friendly structure:
    //
    // [projects]
    // [projects."/path/to/project"]
    // trust_level = "trusted"
    //
    // rather than inline tables like:
    //
    // [projects]
    // "/path/to/project" = { trust_level = "trusted" }
    let project_key = project_path.to_string_lossy().to_string();

    // Ensure top-level `projects` exists as a non-inline, explicit table. If it
    // exists but was previously represented as a non-table (e.g., inline),
    // replace it with an explicit table.
    let mut created_projects_table = false;
    {
        let root = doc.as_table_mut();
        let needs_table = !root.contains_key("projects")
            || root.get("projects").and_then(|i| i.as_table()).is_none();
        if needs_table {
            root.insert("projects", toml_edit::table());
            created_projects_table = true;
        }
    }
    let Some(projects_tbl) = doc["projects"].as_table_mut() else {
        return Err(anyhow::anyhow!(
            "projects table missing after initialization"
        ));
    };

    // If we created the `projects` table ourselves, keep it implicit so we
    // don't render a standalone `[projects]` header.
    if created_projects_table {
        projects_tbl.set_implicit(true);
    }

    // Ensure the per-project entry is its own explicit table. If it exists but
    // is not a table (e.g., an inline table), replace it with an explicit table.
    let needs_proj_table = !projects_tbl.contains_key(project_key.as_str())
        || projects_tbl
            .get(project_key.as_str())
            .and_then(|i| i.as_table())
            .is_none();
    if needs_proj_table {
        projects_tbl.insert(project_key.as_str(), toml_edit::table());
    }
    let Some(proj_tbl) = projects_tbl
        .get_mut(project_key.as_str())
        .and_then(|i| i.as_table_mut())
    else {
        return Err(anyhow::anyhow!("project table missing for {}", project_key));
    };
    proj_tbl.set_implicit(false);
    proj_tbl["trust_level"] = toml_edit::value("trusted");

    // ensure slide_home exists
    std::fs::create_dir_all(slide_home)?;

    // create a tmp_file
    let tmp_file = NamedTempFile::new_in(slide_home)?;
    std::fs::write(tmp_file.path(), doc.to_string())?;

    // atomically move the tmp file into config.toml
    tmp_file.persist(config_path)?;

    Ok(())
}

/// Apply a single dotted-path override onto a TOML value.
fn apply_toml_override(root: &mut TomlValue, path: &str, value: TomlValue) {
    use toml::value::Table;

    let segments: Vec<&str> = path.split('.').collect();
    let mut current = root;

    for (idx, segment) in segments.iter().enumerate() {
        let is_last = idx == segments.len() - 1;

        if is_last {
            match current {
                TomlValue::Table(table) => {
                    table.insert(segment.to_string(), value);
                }
                _ => {
                    let mut table = Table::new();
                    table.insert(segment.to_string(), value);
                    *current = TomlValue::Table(table);
                }
            }
            return;
        }

        // Traverse or create intermediate object.
        match current {
            TomlValue::Table(table) => {
                current = table
                    .entry(segment.to_string())
                    .or_insert_with(|| TomlValue::Table(Table::new()));
            }
            _ => {
                *current = TomlValue::Table(Table::new());
                if let TomlValue::Table(tbl) = current {
                    current = tbl
                        .entry(segment.to_string())
                        .or_insert_with(|| TomlValue::Table(Table::new()));
                }
            }
        }
    }
}

/// Base config deserialized from ~/.slide/config.toml.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct ConfigToml {
    /// Optional override of model selection.
    pub model: Option<String>,

    /// Provider to use from the model_providers map.
    pub model_provider: Option<String>,

    /// Size of the context window for the model, in tokens.
    pub model_context_window: Option<u64>,

    /// Maximum number of output tokens.
    pub model_max_output_tokens: Option<u64>,

    /// Default approval policy for executing commands.
    pub approval_policy: Option<AskForApproval>,

    #[serde(default)]
    pub shell_environment_policy: ShellEnvironmentPolicyToml,

    /// Sandbox mode to use.
    pub sandbox_mode: Option<SandboxMode>,

    /// Sandbox configuration to apply if `sandbox` is `WorkspaceWrite`.
    pub sandbox_workspace_write: Option<SandboxWorkspaceWrite>,

    /// Disable server-side response storage (sends the full conversation
    /// context with every request). Currently necessary for OpenAI customers
    /// who have opted into Zero Data Retention (ZDR).
    pub disable_response_storage: Option<bool>,

    /// Optional external command to spawn for end-user notifications.
    #[serde(default)]
    pub notify: Option<Vec<String>>,

    /// System instructions.
    pub instructions: Option<String>,

    /// Definition for MCP servers that Slide can reach out to for tool calls.
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// User-defined provider entries that extend/override the built-in list.
    #[serde(default)]
    pub model_providers: HashMap<String, ModelProviderInfo>,

    /// Maximum number of bytes to include from an AGENTS.md project doc file.
    pub project_doc_max_bytes: Option<usize>,

    /// Profile to use from the `profiles` map.
    pub profile: Option<String>,

    /// Named profiles to facilitate switching between different configurations.
    #[serde(default)]
    pub profiles: HashMap<String, ConfigProfile>,

    /// Settings that govern if and what will be written to `~/.slide/history.jsonl`.
    #[serde(default)]
    pub history: Option<History>,

    /// Optional URI-based file opener. If set, citations to files in the model
    /// output will be hyperlinked using the specified URI scheme.
    pub file_opener: Option<UriBasedFileOpener>,

    /// Collection of settings that are specific to the TUI.
    pub tui: Option<Tui>,

    /// When set to `true`, `AgentReasoning` events will be hidden from the
    /// UI/output. Defaults to `false`.
    pub hide_agent_reasoning: Option<bool>,

    /// When set to `true`, `AgentReasoningRawContentEvent` events will be shown in the UI/output.
    /// Defaults to `false`.
    pub show_raw_agent_reasoning: Option<bool>,

    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    /// Optional verbosity control for GPT-5 models (Responses API `text.verbosity`).
    pub model_verbosity: Option<Verbosity>,

    /// Override to force-enable reasoning summaries for the configured model.
    pub model_supports_reasoning_summaries: Option<bool>,

    /// Base URL for requests to ChatGPT (as opposed to the OpenAI API).
    pub chatgpt_base_url: Option<String>,

    /// Experimental rollout resume path (absolute path to .jsonl; undocumented).
    pub experimental_resume: Option<PathBuf>,

    /// Experimental path to a file whose contents replace the built-in BASE_INSTRUCTIONS.
    pub experimental_instructions_file: Option<PathBuf>,

    pub experimental_use_exec_command_tool: Option<bool>,

    /// The value for the `originator` header included with Responses API requests.
    pub responses_originator_header_internal_override: Option<String>,

    pub projects: Option<HashMap<String, ProjectConfig>>,

    /// If set to `true`, the API key will be signed with the `originator` header.
    pub preferred_auth_method: Option<AuthMode>,

    /// Nested tools section for feature toggles
    pub tools: Option<ToolsToml>,

    /// When true, disables burst-paste detection for typed input entirely.
    /// All characters are inserted as they are received, and no buffering
    /// or placeholder replacement will occur for fast keypress bursts.
    pub disable_paste_burst: Option<bool>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfig {
    pub trust_level: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct ToolsToml {
    #[serde(default, alias = "web_search_request")]
    pub web_search: Option<bool>,

    /// Enable the `view_image` tool that lets the agent attach local images.
    #[serde(default)]
    pub view_image: Option<bool>,
}

impl ConfigToml {
    /// Derive the effective sandbox policy from the configuration.
    fn derive_sandbox_policy(&self, sandbox_mode_override: Option<SandboxMode>) -> SandboxPolicy {
        let resolved_sandbox_mode = sandbox_mode_override
            .or(self.sandbox_mode)
            .unwrap_or_default();
        match resolved_sandbox_mode {
            SandboxMode::ReadOnly => SandboxPolicy::new_read_only_policy(),
            SandboxMode::WorkspaceWrite => match self.sandbox_workspace_write.as_ref() {
                Some(SandboxWorkspaceWrite {
                    writable_roots,
                    network_access,
                    exclude_tmpdir_env_var,
                    exclude_slash_tmp,
                }) => SandboxPolicy::WorkspaceWrite {
                    writable_roots: writable_roots.clone(),
                    network_access: *network_access,
                    exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
                    exclude_slash_tmp: *exclude_slash_tmp,
                },
                None => SandboxPolicy::new_workspace_write_policy(),
            },
            SandboxMode::DangerFullAccess => SandboxPolicy::DangerFullAccess,
        }
    }

    pub fn is_cwd_trusted(&self, resolved_cwd: &Path) -> bool {
        let projects = self.projects.clone().unwrap_or_default();

        let is_path_trusted = |path: &Path| {
            let path_str = path.to_string_lossy().to_string();
            projects
                .get(&path_str)
                .map(|p| p.trust_level.as_deref() == Some("trusted"))
                .unwrap_or(false)
        };

        // Fast path: exact cwd match
        if is_path_trusted(resolved_cwd) {
            return true;
        }

        // If cwd lives inside a git worktree, check whether the root git project
        // (the primary repository working directory) is trusted. This lets
        // worktrees inherit trust from the main project.
        if let Some(root_project) = resolve_root_git_project_for_trust(resolved_cwd) {
            return is_path_trusted(&root_project);
        }

        false
    }

    pub fn get_config_profile(
        &self,
        override_profile: Option<String>,
    ) -> Result<ConfigProfile, std::io::Error> {
        let profile = override_profile.or_else(|| self.profile.clone());

        match profile {
            Some(key) => {
                if let Some(profile) = self.profiles.get(key.as_str()) {
                    return Ok(profile.clone());
                }

                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("config profile `{key}` not found"),
                ))
            }
            None => Ok(ConfigProfile::default()),
        }
    }
}

/// Optional overrides for user configuration (e.g., from CLI flags).
#[derive(Default, Debug, Clone)]
pub struct ConfigOverrides {
    pub model: Option<String>,
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_mode: Option<SandboxMode>,
    pub model_provider: Option<String>,
    pub config_profile: Option<String>,
    pub slide_linux_sandbox_exe: Option<PathBuf>,
    pub base_instructions: Option<String>,
    pub include_plan_tool: Option<bool>,
    pub include_apply_patch_tool: Option<bool>,
    pub include_view_image_tool: Option<bool>,
    pub disable_response_storage: Option<bool>,
    pub show_raw_agent_reasoning: Option<bool>,
    pub tools_web_search_request: Option<bool>,
}

impl Config {
    /// Meant to be used exclusively for tests: `load_with_overrides()` should
    /// be used in all other cases.
    pub fn load_from_base_config_with_overrides(
        cfg: ConfigToml,
        overrides: ConfigOverrides,
        slide_home: PathBuf,
    ) -> std::io::Result<Self> {
        let user_instructions = Self::load_instructions(Some(&slide_home));

        // Destructure ConfigOverrides fully to ensure all overrides are applied.
        let ConfigOverrides {
            model,
            cwd,
            approval_policy,
            sandbox_mode,
            model_provider,
            config_profile: config_profile_key,
            slide_linux_sandbox_exe,
            base_instructions,
            include_plan_tool,
            include_apply_patch_tool,
            include_view_image_tool,
            disable_response_storage,
            show_raw_agent_reasoning,
            tools_web_search_request: override_tools_web_search_request,
        } = overrides;

        let config_profile = match config_profile_key.as_ref().or(cfg.profile.as_ref()) {
            Some(key) => cfg
                .profiles
                .get(key)
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("config profile `{key}` not found"),
                    )
                })?
                .clone(),
            None => ConfigProfile::default(),
        };

        let sandbox_policy = cfg.derive_sandbox_policy(sandbox_mode);

        let mut model_providers = built_in_model_providers();
        // Merge user-defined providers into the built-in list.
        for (key, provider) in cfg.model_providers.into_iter() {
            model_providers.entry(key).or_insert(provider);
        }

        let model_provider_id = model_provider
            .or(config_profile.model_provider)
            .or(cfg.model_provider)
            .unwrap_or_else(|| "openai".to_string());
        let model_provider = model_providers
            .get(&model_provider_id)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Model provider `{model_provider_id}` not found"),
                )
            })?
            .clone();

        let shell_environment_policy = cfg.shell_environment_policy.into();

        let resolved_cwd = {
            use std::env;

            match cwd {
                None => {
                    tracing::info!("cwd not set, using current dir");
                    env::current_dir()?
                }
                Some(p) if p.is_absolute() => p,
                Some(p) => {
                    // Resolve relative path against the current working directory.
                    tracing::info!("cwd is relative, resolving against current dir");
                    let mut current = env::current_dir()?;
                    current.push(p);
                    current
                }
            }
        };

        let history = cfg.history.unwrap_or_default();

        let tools_web_search_request = override_tools_web_search_request
            .or(cfg.tools.as_ref().and_then(|t| t.web_search))
            .unwrap_or(false);

        let include_view_image_tool = include_view_image_tool
            .or(cfg.tools.as_ref().and_then(|t| t.view_image))
            .unwrap_or(true);

        let model = model
            .or(config_profile.model)
            .or(cfg.model)
            .unwrap_or_else(default_model);
        let model_family = find_family_for_model(&model).unwrap_or_else(|| {
            let supports_reasoning_summaries =
                cfg.model_supports_reasoning_summaries.unwrap_or(false);
            ModelFamily {
                slug: model.clone(),
                family: model.clone(),
                needs_special_apply_patch_instructions: false,
                supports_reasoning_summaries,
                uses_local_shell_tool: false,
                apply_patch_tool_type: None,
            }
        });

        let openai_model_info = get_model_info(&model_family);
        let model_context_window = cfg
            .model_context_window
            .or_else(|| openai_model_info.as_ref().map(|info| info.context_window));
        let model_max_output_tokens = cfg.model_max_output_tokens.or_else(|| {
            openai_model_info
                .as_ref()
                .map(|info| info.max_output_tokens)
        });

        let experimental_resume = cfg.experimental_resume;

        // Load base instructions override from a file if specified. If the
        // path is relative, resolve it against the effective cwd so the
        // behaviour matches other path-like config values.
        let experimental_instructions_path = config_profile
            .experimental_instructions_file
            .as_ref()
            .or(cfg.experimental_instructions_file.as_ref());
        let file_base_instructions =
            Self::get_base_instructions(experimental_instructions_path, &resolved_cwd)?;
        let base_instructions = base_instructions.or(file_base_instructions);

        let responses_originator_header: String = cfg
            .responses_originator_header_internal_override
            .unwrap_or(DEFAULT_RESPONSES_ORIGINATOR_HEADER.to_owned());

        let config = Self {
            model,
            model_family,
            model_context_window,
            model_max_output_tokens,
            model_provider_id,
            model_provider,
            cwd: resolved_cwd,
            approval_policy: approval_policy
                .or(config_profile.approval_policy)
                .or(cfg.approval_policy)
                .unwrap_or_else(AskForApproval::default),
            sandbox_policy,
            shell_environment_policy,
            disable_response_storage: config_profile
                .disable_response_storage
                .or(cfg.disable_response_storage)
                .or(disable_response_storage)
                .unwrap_or(false),
            notify: cfg.notify,
            user_instructions,
            base_instructions,
            mcp_servers: cfg.mcp_servers,
            model_providers,
            project_doc_max_bytes: cfg.project_doc_max_bytes.unwrap_or(PROJECT_DOC_MAX_BYTES),
            slide_home,
            history,
            file_opener: cfg.file_opener.unwrap_or(UriBasedFileOpener::VsCode),
            tui: cfg.tui.unwrap_or_default(),
            slide_linux_sandbox_exe,

            hide_agent_reasoning: cfg.hide_agent_reasoning.unwrap_or(false),
            show_raw_agent_reasoning: cfg
                .show_raw_agent_reasoning
                .or(show_raw_agent_reasoning)
                .unwrap_or(false),
            model_reasoning_effort: config_profile
                .model_reasoning_effort
                .or(cfg.model_reasoning_effort)
                .unwrap_or_default(),
            model_reasoning_summary: config_profile
                .model_reasoning_summary
                .or(cfg.model_reasoning_summary)
                .unwrap_or_default(),
            model_verbosity: config_profile.model_verbosity.or(cfg.model_verbosity),
            chatgpt_base_url: config_profile
                .chatgpt_base_url
                .or(cfg.chatgpt_base_url)
                .unwrap_or("https://chatgpt.com/backend-api/".to_string()),

            experimental_resume,
            include_plan_tool: include_plan_tool.unwrap_or(false),
            include_apply_patch_tool: include_apply_patch_tool.unwrap_or(false),
            tools_web_search_request,
            responses_originator_header,
            preferred_auth_method: cfg.preferred_auth_method.unwrap_or(AuthMode::ChatGPT),
            use_experimental_streamable_shell_tool: cfg
                .experimental_use_exec_command_tool
                .unwrap_or(false),
            include_view_image_tool,
            disable_paste_burst: cfg.disable_paste_burst.unwrap_or(false),
        };
        Ok(config)
    }

    fn load_instructions(slide_dir: Option<&Path>) -> Option<String> {
        let mut p = match slide_dir {
            Some(p) => p.to_path_buf(),
            None => return None,
        };

        p.push("AGENTS.md");
        std::fs::read_to_string(&p).ok().and_then(|s| {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        })
    }

    fn get_base_instructions(
        path: Option<&PathBuf>,
        cwd: &Path,
    ) -> std::io::Result<Option<String>> {
        let p = match path.as_ref() {
            None => return Ok(None),
            Some(p) => p,
        };

        // Resolve relative paths against the provided cwd to make CLI
        // overrides consistent regardless of where the process was launched
        // from.
        let full_path = if p.is_relative() {
            cwd.join(p)
        } else {
            p.to_path_buf()
        };

        let contents = std::fs::read_to_string(&full_path).map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!(
                    "failed to read experimental instructions file {}: {e}",
                    full_path.display()
                ),
            )
        })?;

        let s = contents.trim().to_string();
        if s.is_empty() {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "experimental instructions file is empty: {}",
                    full_path.display()
                ),
            ))
        } else {
            Ok(Some(s))
        }
    }
}

fn default_model() -> String {
    OPENAI_DEFAULT_MODEL.to_string()
}

/// Returns the path to the Slide configuration directory, which can be
/// specified by the `SLIDE_HOME` environment variable. If not set, defaults to
/// `~/.slide`.
///
/// - If `SLIDE_HOME` is set, the value will be canonicalized and this
///   function will Err if the path does not exist.
/// - If `SLIDE_HOME` is not set, this function does not verify that the
///   directory exists.
pub fn find_slide_home() -> std::io::Result<PathBuf> {
    // Honor the `SLIDE_HOME` environment variable when it is set to allow users
    // (and tests) to override the default location.
    if let Ok(val) = std::env::var("SLIDE_HOME")
        && !val.is_empty()
    {
        return PathBuf::from(val).canonicalize();
    }

    let mut p = home_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not find home directory",
        )
    })?;
    p.push(".slide");
    Ok(p)
}

/// Returns the path to the folder where Slide logs are stored. Does not verify
/// that the directory exists.
pub fn log_dir(cfg: &Config) -> std::io::Result<PathBuf> {
    let mut p = cfg.slide_home.clone();
    p.push("log");
    Ok(p)
}

#[cfg(test)]
mod tests {
    use crate::config_types::HistoryPersistence;

    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn test_toml_parsing() {
        let history_with_persistence = r#"
[history]
persistence = "save-all"
"#;
        let history_with_persistence_cfg = toml::from_str::<ConfigToml>(history_with_persistence)
            .expect("TOML deserialization should succeed");
        assert_eq!(
            Some(History {
                persistence: HistoryPersistence::SaveAll,
                max_bytes: None,
            }),
            history_with_persistence_cfg.history
        );

        let history_no_persistence = r#"
[history]
persistence = "none"
"#;

        let history_no_persistence_cfg = toml::from_str::<ConfigToml>(history_no_persistence)
            .expect("TOML deserialization should succeed");
        assert_eq!(
            Some(History {
                persistence: HistoryPersistence::None,
                max_bytes: None,
            }),
            history_no_persistence_cfg.history
        );
    }

    #[test]
    fn test_sandbox_config_parsing() {
        let sandbox_full_access = r#"
sandbox_mode = "danger-full-access"

[sandbox_workspace_write]
network_access = false  # This should be ignored.
"#;
        let sandbox_full_access_cfg = toml::from_str::<ConfigToml>(sandbox_full_access)
            .expect("TOML deserialization should succeed");
        let sandbox_mode_override = None;
        assert_eq!(
            SandboxPolicy::DangerFullAccess,
            sandbox_full_access_cfg.derive_sandbox_policy(sandbox_mode_override)
        );

        let sandbox_read_only = r#"
sandbox_mode = "read-only"

[sandbox_workspace_write]
network_access = true  # This should be ignored.
"#;

        let sandbox_read_only_cfg = toml::from_str::<ConfigToml>(sandbox_read_only)
            .expect("TOML deserialization should succeed");
        let sandbox_mode_override = None;
        assert_eq!(
            SandboxPolicy::ReadOnly,
            sandbox_read_only_cfg.derive_sandbox_policy(sandbox_mode_override)
        );

        let sandbox_workspace_write = r#"
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
writable_roots = [
    "/my/workspace",
]
exclude_tmpdir_env_var = true
exclude_slash_tmp = true
"#;

        let sandbox_workspace_write_cfg = toml::from_str::<ConfigToml>(sandbox_workspace_write)
            .expect("TOML deserialization should succeed");
        let sandbox_mode_override = None;
        assert_eq!(
            SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![PathBuf::from("/my/workspace")],
                network_access: false,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            },
            sandbox_workspace_write_cfg.derive_sandbox_policy(sandbox_mode_override)
        );
    }

    struct PrecedenceTestFixture {
        cwd: TempDir,
        slide_home: TempDir,
        cfg: ConfigToml,
        model_provider_map: HashMap<String, ModelProviderInfo>,
        openai_provider: ModelProviderInfo,
        openai_chat_completions_provider: ModelProviderInfo,
    }

    impl PrecedenceTestFixture {
        fn cwd(&self) -> PathBuf {
            self.cwd.path().to_path_buf()
        }

        fn slide_home(&self) -> PathBuf {
            self.slide_home.path().to_path_buf()
        }
    }

    fn create_test_fixture() -> std::io::Result<PrecedenceTestFixture> {
        let toml = r#"
model = "o3"
approval_policy = "untrusted"
disable_response_storage = false

# Can be used to determine which profile to use if not specified by
# `ConfigOverrides`.
profile = "gpt3"

[model_providers.openai-chat-completions]
name = "OpenAI using Chat Completions"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
wire_api = "chat"
request_max_retries = 4            # retry failed HTTP requests
stream_max_retries = 10            # retry dropped SSE streams
stream_idle_timeout_ms = 300000    # 5m idle timeout

[profiles.o3]
model = "o3"
model_provider = "openai"
approval_policy = "never"
model_reasoning_effort = "high"
model_reasoning_summary = "detailed"

[profiles.gpt3]
model = "gpt-3.5-turbo"
model_provider = "openai-chat-completions"

[profiles.zdr]
model = "o3"
model_provider = "openai"
approval_policy = "on-failure"
disable_response_storage = true
"#;

        let cfg: ConfigToml = toml::from_str(toml).expect("TOML deserialization should succeed");

        // Use a temporary directory for the cwd so it does not contain an
        // AGENTS.md file.
        let cwd_temp_dir = TempDir::new().unwrap();
        let cwd = cwd_temp_dir.path().to_path_buf();
        // Make it look like a Git repo so it does not search for AGENTS.md in
        // a parent folder, either.
        std::fs::write(cwd.join(".git"), "gitdir: nowhere")?;

        let slide_home_temp_dir = TempDir::new().unwrap();

        let openai_chat_completions_provider = ModelProviderInfo {
            name: "OpenAI using Chat Completions".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            wire_api: crate::WireApi::Chat,
            env_key_instructions: None,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: Some(4),
            stream_max_retries: Some(10),
            stream_idle_timeout_ms: Some(300_000),
            requires_openai_auth: false,
        };
        let model_provider_map = {
            let mut model_provider_map = built_in_model_providers();
            model_provider_map.insert(
                "openai-chat-completions".to_string(),
                openai_chat_completions_provider.clone(),
            );
            model_provider_map
        };

        let openai_provider = model_provider_map
            .get("openai")
            .expect("openai provider should exist")
            .clone();

        Ok(PrecedenceTestFixture {
            cwd: cwd_temp_dir,
            slide_home: slide_home_temp_dir,
            cfg,
            model_provider_map,
            openai_provider,
            openai_chat_completions_provider,
        })
    }

    /// Users can specify config values at multiple levels that have the
    /// following precedence:
    ///
    /// 1. custom command-line argument, e.g. `--model o3`
    /// 2. as part of a profile, where the `--profile` is specified via a CLI
    ///    (or in the config file itself)
    /// 3. as an entry in `config.toml`, e.g. `model = "o3"`
    /// 4. the default value for a required field defined in code, e.g.,
    ///    `crate::flags::OPENAI_DEFAULT_MODEL`
    ///
    /// Note that profiles are the recommended way to specify a group of
    /// configuration options together.
    #[test]
    fn test_precedence_fixture_with_o3_profile() -> std::io::Result<()> {
        let fixture = create_test_fixture()?;

        let o3_profile_overrides = ConfigOverrides {
            config_profile: Some("o3".to_string()),
            cwd: Some(fixture.cwd()),
            ..Default::default()
        };
        let o3_profile_config: Config = Config::load_from_base_config_with_overrides(
            fixture.cfg.clone(),
            o3_profile_overrides,
            fixture.slide_home(),
        )?;
        assert_eq!(
            Config {
                model: "o3".to_string(),
                model_family: find_family_for_model("o3").expect("known model slug"),
                model_context_window: Some(200_000),
                model_max_output_tokens: Some(100_000),
                model_provider_id: "openai".to_string(),
                model_provider: fixture.openai_provider.clone(),
                approval_policy: AskForApproval::Never,
                sandbox_policy: SandboxPolicy::new_read_only_policy(),
                shell_environment_policy: ShellEnvironmentPolicy::default(),
                disable_response_storage: false,
                user_instructions: None,
                notify: None,
                cwd: fixture.cwd(),
                mcp_servers: HashMap::new(),
                model_providers: fixture.model_provider_map.clone(),
                project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
                slide_home: fixture.slide_home(),
                history: History::default(),
                file_opener: UriBasedFileOpener::VsCode,
                tui: Tui::default(),
                slide_linux_sandbox_exe: None,
                hide_agent_reasoning: false,
                show_raw_agent_reasoning: false,
                model_reasoning_effort: ReasoningEffort::High,
                model_reasoning_summary: ReasoningSummary::Detailed,
                model_verbosity: None,
                chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
                experimental_resume: None,
                base_instructions: None,
                include_plan_tool: false,
                include_apply_patch_tool: false,
                tools_web_search_request: false,
                responses_originator_header: "slide_cli_rs".to_string(),
                preferred_auth_method: AuthMode::ChatGPT,
                use_experimental_streamable_shell_tool: false,
                include_view_image_tool: true,
                disable_paste_burst: false,
            },
            o3_profile_config
        );
        Ok(())
    }

    #[test]
    fn test_precedence_fixture_with_gpt3_profile() -> std::io::Result<()> {
        let fixture = create_test_fixture()?;

        let gpt3_profile_overrides = ConfigOverrides {
            config_profile: Some("gpt3".to_string()),
            cwd: Some(fixture.cwd()),
            ..Default::default()
        };
        let gpt3_profile_config = Config::load_from_base_config_with_overrides(
            fixture.cfg.clone(),
            gpt3_profile_overrides,
            fixture.slide_home(),
        )?;
        let expected_gpt3_profile_config = Config {
            model: "gpt-3.5-turbo".to_string(),
            model_family: find_family_for_model("gpt-3.5-turbo").expect("known model slug"),
            model_context_window: Some(16_385),
            model_max_output_tokens: Some(4_096),
            model_provider_id: "openai-chat-completions".to_string(),
            model_provider: fixture.openai_chat_completions_provider.clone(),
            approval_policy: AskForApproval::UnlessTrusted,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            shell_environment_policy: ShellEnvironmentPolicy::default(),
            disable_response_storage: false,
            user_instructions: None,
            notify: None,
            cwd: fixture.cwd(),
            mcp_servers: HashMap::new(),
            model_providers: fixture.model_provider_map.clone(),
            project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
            slide_home: fixture.slide_home(),
            history: History::default(),
            file_opener: UriBasedFileOpener::VsCode,
            tui: Tui::default(),
            slide_linux_sandbox_exe: None,
            hide_agent_reasoning: false,
            show_raw_agent_reasoning: false,
            model_reasoning_effort: ReasoningEffort::default(),
            model_reasoning_summary: ReasoningSummary::default(),
            model_verbosity: None,
            chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
            experimental_resume: None,
            base_instructions: None,
            include_plan_tool: false,
            include_apply_patch_tool: false,
            tools_web_search_request: false,
            responses_originator_header: "slide_cli_rs".to_string(),
            preferred_auth_method: AuthMode::ChatGPT,
            use_experimental_streamable_shell_tool: false,
            include_view_image_tool: true,
            disable_paste_burst: false,
        };

        assert_eq!(expected_gpt3_profile_config, gpt3_profile_config);

        // Verify that loading without specifying a profile in ConfigOverrides
        // uses the default profile from the config file (which is "gpt3").
        let default_profile_overrides = ConfigOverrides {
            cwd: Some(fixture.cwd()),
            ..Default::default()
        };

        let default_profile_config = Config::load_from_base_config_with_overrides(
            fixture.cfg.clone(),
            default_profile_overrides,
            fixture.slide_home(),
        )?;

        assert_eq!(expected_gpt3_profile_config, default_profile_config);
        Ok(())
    }

    #[test]
    fn test_precedence_fixture_with_zdr_profile() -> std::io::Result<()> {
        let fixture = create_test_fixture()?;

        let zdr_profile_overrides = ConfigOverrides {
            config_profile: Some("zdr".to_string()),
            cwd: Some(fixture.cwd()),
            ..Default::default()
        };
        let zdr_profile_config = Config::load_from_base_config_with_overrides(
            fixture.cfg.clone(),
            zdr_profile_overrides,
            fixture.slide_home(),
        )?;
        let expected_zdr_profile_config = Config {
            model: "o3".to_string(),
            model_family: find_family_for_model("o3").expect("known model slug"),
            model_context_window: Some(200_000),
            model_max_output_tokens: Some(100_000),
            model_provider_id: "openai".to_string(),
            model_provider: fixture.openai_provider.clone(),
            approval_policy: AskForApproval::OnFailure,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            shell_environment_policy: ShellEnvironmentPolicy::default(),
            disable_response_storage: true,
            user_instructions: None,
            notify: None,
            cwd: fixture.cwd(),
            mcp_servers: HashMap::new(),
            model_providers: fixture.model_provider_map.clone(),
            project_doc_max_bytes: PROJECT_DOC_MAX_BYTES,
            slide_home: fixture.slide_home(),
            history: History::default(),
            file_opener: UriBasedFileOpener::VsCode,
            tui: Tui::default(),
            slide_linux_sandbox_exe: None,
            hide_agent_reasoning: false,
            show_raw_agent_reasoning: false,
            model_reasoning_effort: ReasoningEffort::default(),
            model_reasoning_summary: ReasoningSummary::default(),
            model_verbosity: None,
            chatgpt_base_url: "https://chatgpt.com/backend-api/".to_string(),
            experimental_resume: None,
            base_instructions: None,
            include_plan_tool: false,
            include_apply_patch_tool: false,
            tools_web_search_request: false,
            responses_originator_header: "slide_cli_rs".to_string(),
            preferred_auth_method: AuthMode::ChatGPT,
            use_experimental_streamable_shell_tool: false,
            include_view_image_tool: true,
            disable_paste_burst: false,
        };

        assert_eq!(expected_zdr_profile_config, zdr_profile_config);

        Ok(())
    }

    #[test]
    fn test_set_project_trusted_writes_explicit_tables() -> anyhow::Result<()> {
        let slide_home = TempDir::new().unwrap();
        let project_dir = TempDir::new().unwrap();

        // Call the function under test
        set_project_trusted(slide_home.path(), project_dir.path())?;

        // Read back the generated config.toml and assert exact contents
        let config_path = slide_home.path().join(CONFIG_TOML_FILE);
        let contents = std::fs::read_to_string(&config_path)?;

        let raw_path = project_dir.path().to_string_lossy();
        let path_str = if raw_path.contains('\\') {
            format!("'{raw_path}'")
        } else {
            format!("\"{raw_path}\"")
        };
        let expected = format!(
            r#"[projects.{path_str}]
trust_level = "trusted"
"#
        );
        assert_eq!(contents, expected);

        Ok(())
    }

    #[test]
    fn test_set_project_trusted_converts_inline_to_explicit() -> anyhow::Result<()> {
        let slide_home = TempDir::new().unwrap();
        let project_dir = TempDir::new().unwrap();

        // Seed config.toml with an inline project entry under [projects]
        let config_path = slide_home.path().join(CONFIG_TOML_FILE);
        let raw_path = project_dir.path().to_string_lossy();
        let path_str = if raw_path.contains('\\') {
            format!("'{raw_path}'")
        } else {
            format!("\"{raw_path}\"")
        };
        // Use a quoted key so backslashes don't require escaping on Windows
        let initial = format!(
            r#"[projects]
{path_str} = {{ trust_level = "untrusted" }}
"#
        );
        std::fs::create_dir_all(slide_home.path())?;
        std::fs::write(&config_path, initial)?;

        // Run the function; it should convert to explicit tables and set trusted
        set_project_trusted(slide_home.path(), project_dir.path())?;

        let contents = std::fs::read_to_string(&config_path)?;

        // Assert exact output after conversion to explicit table
        let expected = format!(
            r#"[projects]

[projects.{path_str}]
trust_level = "trusted"
"#
        );
        assert_eq!(contents, expected);

        Ok(())
    }

    // No test enforcing the presence of a standalone [projects] header.
}
```

#### slide-rs/core/src/spawn.rs
```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Child;
use tokio::process::Command;
use tracing::trace;

use crate::protocol::SandboxPolicy;

/// Experimental environment variable that will be set to some non-empty value
/// if both of the following are true:
///
/// 1. The process was spawned by Slide as part of a shell tool call.
/// 2. SandboxPolicy.has_full_network_access() was false for the tool call.
///
/// We may try to have just one environment variable for all sandboxing
/// attributes, so this may change in the future.
pub const SLIDE_SANDBOX_NETWORK_DISABLED_ENV_VAR: &str = "SLIDE_SANDBOX_NETWORK_DISABLED";

/// Should be set when the process is spawned under a sandbox. Currently, the
/// value is "seatbelt" for macOS, but it may change in the future to
/// accommodate sandboxing configuration and other sandboxing mechanisms.
pub const SLIDE_SANDBOX_ENV_VAR: &str = "SLIDE_SANDBOX";

#[derive(Debug, Clone, Copy)]
pub enum StdioPolicy {
    RedirectForShellTool,
    Inherit,
}

/// Spawns the appropriate child process for the ExecParams and SandboxPolicy,
/// ensuring the args and environment variables used to create the `Command`
/// (and `Child`) honor the configuration.
///
/// For now, we take `SandboxPolicy` as a parameter to spawn_child() because
/// we need to determine whether to set the
/// `SLIDE_SANDBOX_NETWORK_DISABLED_ENV_VAR` environment variable.
pub(crate) async fn spawn_child_async(
    program: PathBuf,
    args: Vec<String>,
    #[cfg_attr(not(unix), allow(unused_variables))] arg0: Option<&str>,
    cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> std::io::Result<Child> {
    trace!(
        "spawn_child_async: {program:?} {args:?} {arg0:?} {cwd:?} {sandbox_policy:?} {stdio_policy:?} {env:?}"
    );

    let mut cmd = Command::new(&program);
    #[cfg(unix)]
    cmd.arg0(arg0.map_or_else(|| program.to_string_lossy().to_string(), String::from));
    cmd.args(args);
    cmd.current_dir(cwd);
    cmd.env_clear();
    cmd.envs(env);

    if !sandbox_policy.has_full_network_access() {
        cmd.env(SLIDE_SANDBOX_NETWORK_DISABLED_ENV_VAR, "1");
    }

    // If this Slide process dies (including being killed via SIGKILL), we want
    // any child processes that were spawned as part of a `"shell"` tool call
    // to also be terminated.

    // This relies on prctl(2), so it only works on Linux.
    #[cfg(target_os = "linux")]
    unsafe {
        cmd.pre_exec(|| {
            // This prctl call effectively requests, "deliver SIGTERM when my
            // current parent dies."
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == -1 {
                return Err(std::io::Error::last_os_error());
            }

            // Though if there was a race condition and this pre_exec() block is
            // run _after_ the parent (i.e., the Slide process) has already
            // exited, then the parent is the _init_ process (which will never
            // die), so we should just terminate the child process now.
            if libc::getppid() == 1 {
                libc::raise(libc::SIGTERM);
            }
            Ok(())
        });
    }

    match stdio_policy {
        StdioPolicy::RedirectForShellTool => {
            // Do not create a file descriptor for stdin because otherwise some
            // commands may hang forever waiting for input. For example, ripgrep has
            // a heuristic where it may try to read from stdin as explained here:
            // https://github.com/BurntSushi/ripgrep/blob/e2362d4d5185d02fa857bf381e7bd52e66fafc73/crates/core/flags/hiargs.rs#L1101-L1103
            cmd.stdin(Stdio::null());

            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        }
        StdioPolicy::Inherit => {
            // Inherit stdin, stdout, and stderr from the parent process.
            cmd.stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
        }
    }

    cmd.kill_on_drop(true).spawn()
}
```

#### slide-rs/core/src/landlock.rs
```rust
use crate::protocol::SandboxPolicy;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

/// Spawn a shell tool command under the Linux Landlock+seccomp sandbox helper
/// (slide-linux-sandbox).
///
/// Unlike macOS Seatbelt where we directly embed the policy text, the Linux
/// helper accepts a list of `--sandbox-permission`/`-s` flags mirroring the
/// public CLI. We convert the internal [`SandboxPolicy`] representation into
/// the equivalent CLI options.
pub async fn spawn_command_under_linux_sandbox<P>(
    slide_linux_sandbox_exe: P,
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: PathBuf,
    stdio_policy: StdioPolicy,
    env: HashMap<String, String>,
) -> std::io::Result<Child>
where
    P: AsRef<Path>,
{
    let args = create_linux_sandbox_command_args(command, sandbox_policy, &cwd);
    let arg0 = Some("slide-linux-sandbox");
    spawn_child_async(
        slide_linux_sandbox_exe.as_ref().to_path_buf(),
        args,
        arg0,
        cwd,
        sandbox_policy,
        stdio_policy,
        env,
    )
    .await
}

/// Converts the sandbox policy into the CLI invocation for `slide-linux-sandbox`.
fn create_linux_sandbox_command_args(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> Vec<String> {
    #[expect(clippy::expect_used)]
    let sandbox_policy_cwd = cwd.to_str().expect("cwd must be valid UTF-8").to_string();

    #[expect(clippy::expect_used)]
    let sandbox_policy_json =
        serde_json::to_string(sandbox_policy).expect("Failed to serialize SandboxPolicy to JSON");

    let mut linux_cmd: Vec<String> = vec![
        sandbox_policy_cwd,
        sandbox_policy_json,
        // Separator so that command arguments starting with `-` are not parsed as
        // options of the helper itself.
        "--".to_string(),
    ];

    // Append the original tool command.
    linux_cmd.extend(command);

    linux_cmd
}
```

#### slide-rs/core/src/exec.rs
```rust
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;
use std::time::Instant;

use async_channel::Sender;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::process::Child;

use crate::error::SlideErr;
use crate::error::Result;
use crate::error::SandboxErr;
use crate::landlock::spawn_command_under_linux_sandbox;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandOutputDeltaEvent;
use crate::protocol::ExecOutputStream;
use crate::protocol::SandboxPolicy;
use crate::seatbelt::spawn_command_under_seatbelt;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use serde_bytes::ByteBuf;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;

// Hardcode these since it does not seem worth including the libc crate just
// for these.
const SIGKILL_CODE: i32 = 9;
const TIMEOUT_CODE: i32 = 64;
const EXIT_CODE_SIGNAL_BASE: i32 = 128; // conventional shell: 128 + signal

// I/O buffer sizing
const READ_CHUNK_SIZE: usize = 8192; // bytes per read
const AGGREGATE_BUFFER_INITIAL_CAPACITY: usize = 8 * 1024; // 8 KiB

/// Limit the number of ExecCommandOutputDelta events emitted per exec call.
/// Aggregation still collects full output; only the live event stream is capped.
pub(crate) const MAX_EXEC_OUTPUT_DELTAS_PER_CALL: usize = 10_000;

#[derive(Debug, Clone)]
pub struct ExecParams {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub timeout_ms: Option<u64>,
    pub env: HashMap<String, String>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
}

impl ExecParams {
    pub fn timeout_duration(&self) -> Duration {
        Duration::from_millis(self.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SandboxType {
    None,

    /// Only available on macOS.
    MacosSeatbelt,

    /// Only available on Linux.
    LinuxSeccomp,
}

#[derive(Clone)]
pub struct StdoutStream {
    pub sub_id: String,
    pub call_id: String,
    pub tx_event: Sender<Event>,
}

pub async fn process_exec_tool_call(
    params: ExecParams,
    sandbox_type: SandboxType,
    sandbox_policy: &SandboxPolicy,
    slide_linux_sandbox_exe: &Option<PathBuf>,
    stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    let start = Instant::now();

    let raw_output_result: std::result::Result<RawExecToolCallOutput, SlideErr> = match sandbox_type
    {
        SandboxType::None => exec(params, sandbox_policy, stdout_stream.clone()).await,
        SandboxType::MacosSeatbelt => {
            let timeout = params.timeout_duration();
            let ExecParams {
                command, cwd, env, ..
            } = params;
            let child = spawn_command_under_seatbelt(
                command,
                sandbox_policy,
                cwd,
                StdioPolicy::RedirectForShellTool,
                env,
            )
            .await?;
            consume_truncated_output(child, timeout, stdout_stream.clone()).await
        }
        SandboxType::LinuxSeccomp => {
            let timeout = params.timeout_duration();
            let ExecParams {
                command, cwd, env, ..
            } = params;

            let slide_linux_sandbox_exe = slide_linux_sandbox_exe
                .as_ref()
                .ok_or(SlideErr::LandlockSandboxExecutableNotProvided)?;
            let child = spawn_command_under_linux_sandbox(
                slide_linux_sandbox_exe,
                command,
                sandbox_policy,
                cwd,
                StdioPolicy::RedirectForShellTool,
                env,
            )
            .await?;

            consume_truncated_output(child, timeout, stdout_stream).await
        }
    };
    let duration = start.elapsed();
    match raw_output_result {
        Ok(raw_output) => {
            let stdout = raw_output.stdout.from_utf8_lossy();
            let stderr = raw_output.stderr.from_utf8_lossy();

            #[cfg(target_family = "unix")]
            match raw_output.exit_status.signal() {
                Some(TIMEOUT_CODE) => return Err(SlideErr::Sandbox(SandboxErr::Timeout)),
                Some(signal) => {
                    return Err(SlideErr::Sandbox(SandboxErr::Signal(signal)));
                }
                None => {}
            }

            let exit_code = raw_output.exit_status.code().unwrap_or(-1);

            if exit_code != 0 && is_likely_sandbox_denied(sandbox_type, exit_code) {
                return Err(SlideErr::Sandbox(SandboxErr::Denied(
                    exit_code,
                    stdout.text,
                    stderr.text,
                )));
            }

            Ok(ExecToolCallOutput {
                exit_code,
                stdout,
                stderr,
                aggregated_output: raw_output.aggregated_output.from_utf8_lossy(),
                duration,
            })
        }
        Err(err) => {
            tracing::error!("exec error: {err}");
            Err(err)
        }
    }
}

/// We don't have a fully deterministic way to tell if our command failed
/// because of the sandbox - a command in the user's zshrc file might hit an
/// error, but the command itself might fail or succeed for other reasons.
/// For now, we conservatively check for 'command not found' (exit code 127),
/// and can add additional cases as necessary.
fn is_likely_sandbox_denied(sandbox_type: SandboxType, exit_code: i32) -> bool {
    if sandbox_type == SandboxType::None {
        return false;
    }

    // Quick rejects: well-known non-sandbox shell exit codes
    // 127: command not found, 2: misuse of shell builtins
    if exit_code == 127 {
        return false;
    }

    // For all other cases, we assume the sandbox is the cause
    true
}

#[derive(Debug)]
pub struct StreamOutput<T> {
    pub text: T,
    pub truncated_after_lines: Option<u32>,
}
#[derive(Debug)]
struct RawExecToolCallOutput {
    pub exit_status: ExitStatus,
    pub stdout: StreamOutput<Vec<u8>>,
    pub stderr: StreamOutput<Vec<u8>>,
    pub aggregated_output: StreamOutput<Vec<u8>>,
}

impl StreamOutput<String> {
    pub fn new(text: String) -> Self {
        Self {
            text,
            truncated_after_lines: None,
        }
    }
}

impl StreamOutput<Vec<u8>> {
    pub fn from_utf8_lossy(&self) -> StreamOutput<String> {
        StreamOutput {
            text: String::from_utf8_lossy(&self.text).to_string(),
            truncated_after_lines: self.truncated_after_lines,
        }
    }
}

#[inline]
fn append_all(dst: &mut Vec<u8>, src: &[u8]) {
    dst.extend_from_slice(src);
}

#[derive(Debug)]
pub struct ExecToolCallOutput {
    pub exit_code: i32,
    pub stdout: StreamOutput<String>,
    pub stderr: StreamOutput<String>,
    pub aggregated_output: StreamOutput<String>,
    pub duration: Duration,
}

async fn exec(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
) -> Result<RawExecToolCallOutput> {
    let timeout = params.timeout_duration();
    let ExecParams {
        command, cwd, env, ..
    } = params;

    let (program, args) = command.split_first().ok_or_else(|| {
        SlideErr::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command args are empty",
        ))
    })?;
    let arg0 = None;
    let child = spawn_child_async(
        PathBuf::from(program),
        args.into(),
        arg0,
        cwd,
        sandbox_policy,
        StdioPolicy::RedirectForShellTool,
        env,
    )
    .await?;
    consume_truncated_output(child, timeout, stdout_stream).await
}

/// Consumes the output of a child process, truncating it so it is suitable for
/// use as the output of a `shell` tool call. Also enforces specified timeout.
async fn consume_truncated_output(
    mut child: Child,
    timeout: Duration,
    stdout_stream: Option<StdoutStream>,
) -> Result<RawExecToolCallOutput> {
    // Both stdout and stderr were configured with `Stdio::piped()`
    // above, therefore `take()` should normally return `Some`.  If it doesn't
    // we treat it as an exceptional I/O error

    let stdout_reader = child.stdout.take().ok_or_else(|| {
        SlideErr::Io(io::Error::other(
            "stdout pipe was unexpectedly not available",
        ))
    })?;
    let stderr_reader = child.stderr.take().ok_or_else(|| {
        SlideErr::Io(io::Error::other(
            "stderr pipe was unexpectedly not available",
        ))
    })?;

    let (agg_tx, agg_rx) = async_channel::unbounded::<Vec<u8>>();

    let stdout_handle = tokio::spawn(read_capped(
        BufReader::new(stdout_reader),
        stdout_stream.clone(),
        false,
        Some(agg_tx.clone()),
    ));
    let stderr_handle = tokio::spawn(read_capped(
        BufReader::new(stderr_reader),
        stdout_stream.clone(),
        true,
        Some(agg_tx.clone()),
    ));

    let exit_status = tokio::select! {
        result = tokio::time::timeout(timeout, child.wait()) => {
            match result {
                Ok(Ok(exit_status)) => exit_status,
                Ok(e) => e?,
                Err(_) => {
                    // timeout
                    child.start_kill()?;
                    // Debatable whether `child.wait().await` should be called here.
                    synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + TIMEOUT_CODE)
                }
            }
        }
        _ = tokio::signal::ctrl_c() => {
            child.start_kill()?;
            synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + SIGKILL_CODE)
        }
    };

    let stdout = stdout_handle.await??;
    let stderr = stderr_handle.await??;

    drop(agg_tx);

    let mut combined_buf = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY);
    while let Ok(chunk) = agg_rx.recv().await {
        append_all(&mut combined_buf, &chunk);
    }
    let aggregated_output = StreamOutput {
        text: combined_buf,
        truncated_after_lines: None,
    };

    Ok(RawExecToolCallOutput {
        exit_status,
        stdout,
        stderr,
        aggregated_output,
    })
}

async fn read_capped<R: AsyncRead + Unpin + Send + 'static>(
    mut reader: R,
    stream: Option<StdoutStream>,
    is_stderr: bool,
    aggregate_tx: Option<Sender<Vec<u8>>>,
) -> io::Result<StreamOutput<Vec<u8>>> {
    let mut buf = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY);
    let mut tmp = [0u8; READ_CHUNK_SIZE];
    let mut emitted_deltas: usize = 0;

    // No caps: append all bytes

    loop {
        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            break;
        }

        if let Some(stream) = &stream
            && emitted_deltas < MAX_EXEC_OUTPUT_DELTAS_PER_CALL
        {
            let chunk = tmp[..n].to_vec();
            let msg = EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                call_id: stream.call_id.clone(),
                stream: if is_stderr {
                    ExecOutputStream::Stderr
                } else {
                    ExecOutputStream::Stdout
                },
                chunk: ByteBuf::from(chunk),
            });
            let event = Event {
                id: stream.sub_id.clone(),
                msg,
            };
            #[allow(clippy::let_unit_value)]
            let _ = stream.tx_event.send(event).await;
            emitted_deltas += 1;
        }

        if let Some(tx) = &aggregate_tx {
            let _ = tx.send(tmp[..n].to_vec()).await;
        }

        append_all(&mut buf, &tmp[..n]);
        // Continue reading to EOF to avoid back-pressure
    }

    Ok(StreamOutput {
        text: buf,
        truncated_after_lines: None,
    })
}

#[cfg(unix)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code)
}

#[cfg(windows)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    #[expect(clippy::unwrap_used)]
    std::process::ExitStatus::from_raw(code.try_into().unwrap())
}
```

#### slide-rs/core/src/exec_env.rs
```rust
use crate::config_types::EnvironmentVariablePattern;
use crate::config_types::ShellEnvironmentPolicy;
use crate::config_types::ShellEnvironmentPolicyInherit;
use std::collections::HashMap;
use std::collections::HashSet;

/// Construct an environment map based on the rules in the specified policy. The
/// resulting map can be passed directly to `Command::envs()` after calling
/// `env_clear()` to ensure no unintended variables are leaked to the spawned
/// process.
///
/// The derivation follows the algorithm documented in the struct-level comment
/// for [`ShellEnvironmentPolicy`].
pub fn create_env(policy: &ShellEnvironmentPolicy) -> HashMap<String, String> {
    populate_env(std::env::vars(), policy)
}

fn populate_env<I>(vars: I, policy: &ShellEnvironmentPolicy) -> HashMap<String, String>
where
    I: IntoIterator<Item = (String, String)>,
{
    // Step 1 – determine the starting set of variables based on the
    // `inherit` strategy.
    let mut env_map: HashMap<String, String> = match policy.inherit {
        ShellEnvironmentPolicyInherit::All => vars.into_iter().collect(),
        ShellEnvironmentPolicyInherit::None => HashMap::new(),
        ShellEnvironmentPolicyInherit::Core => {
            const CORE_VARS: &[&str] = &[
                "HOME", "LOGNAME", "PATH", "SHELL", "USER", "USERNAME", "TMPDIR", "TEMP", "TMP",
            ];
            let allow: HashSet<&str> = CORE_VARS.iter().copied().collect();
            vars.into_iter()
                .filter(|(k, _)| allow.contains(k.as_str()))
                .collect()
        }
    };

    // Internal helper – does `name` match **any** pattern in `patterns`?
    let matches_any = |name: &str, patterns: &[EnvironmentVariablePattern]| -> bool {
        patterns.iter().any(|pattern| pattern.matches(name))
    };

    // Step 2 – Apply the default exclude if not disabled.
    if !policy.ignore_default_excludes {
        let default_excludes = vec![
            EnvironmentVariablePattern::new_case_insensitive("*KEY*"),
            EnvironmentVariablePattern::new_case_insensitive("*SECRET*"),
            EnvironmentVariablePattern::new_case_insensitive("*TOKEN*"),
        ];
        env_map.retain(|k, _| !matches_any(k, &default_excludes));
    }

    // Step 3 – Apply custom excludes.
    if !policy.exclude.is_empty() {
        env_map.retain(|k, _| !matches_any(k, &policy.exclude));
    }

    // Step 4 – Apply user-provided overrides.
    for (key, val) in &policy.r#set {
        env_map.insert(key.clone(), val.clone());
    }

    // Step 5 – If include_only is non-empty, keep *only* the matching vars.
    if !policy.include_only.is_empty() {
        env_map.retain(|k, _| matches_any(k, &policy.include_only));
    }

    env_map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_types::ShellEnvironmentPolicyInherit;
    use maplit::hashmap;

    fn make_vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_core_inherit_and_default_excludes() {
        let vars = make_vars(&[
            ("PATH", "/usr/bin"),
            ("HOME", "/home/user"),
            ("API_KEY", "secret"),
            ("SECRET_TOKEN", "t"),
        ]);

        let policy = ShellEnvironmentPolicy::default(); // inherit Core, default excludes on
        let result = populate_env(vars, &policy);

        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
            "HOME".to_string() => "/home/user".to_string(),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_include_only() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("FOO", "bar")]);

        let policy = ShellEnvironmentPolicy {
            // skip default excludes so nothing is removed prematurely
            ignore_default_excludes: true,
            include_only: vec![EnvironmentVariablePattern::new_case_insensitive("*PATH")],
            ..Default::default()
        };

        let result = populate_env(vars, &policy);

        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_set_overrides() {
        let vars = make_vars(&[("PATH", "/usr/bin")]);

        let mut policy = ShellEnvironmentPolicy {
            ignore_default_excludes: true,
            ..Default::default()
        };
        policy.r#set.insert("NEW_VAR".to_string(), "42".to_string());

        let result = populate_env(vars, &policy);

        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
            "NEW_VAR".to_string() => "42".to_string(),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_inherit_all() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("FOO", "bar")]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ignore_default_excludes: true, // keep everything
            ..Default::default()
        };

        let result = populate_env(vars.clone(), &policy);
        let expected: HashMap<String, String> = vars.into_iter().collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_inherit_all_with_default_excludes() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("API_KEY", "secret")]);

        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::All,
            ..Default::default()
        };

        let result = populate_env(vars, &policy);
        let expected: HashMap<String, String> = hashmap! {
            "PATH".to_string() => "/usr/bin".to_string(),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn test_inherit_none() {
        let vars = make_vars(&[("PATH", "/usr/bin"), ("HOME", "/home")]);

        let mut policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::None,
            ignore_default_excludes: true,
            ..Default::default()
        };
        policy
            .r#set
            .insert("ONLY_VAR".to_string(), "yes".to_string());

        let result = populate_env(vars, &policy);
        let expected: HashMap<String, String> = hashmap! {
            "ONLY_VAR".to_string() => "yes".to_string(),
        };
        assert_eq!(result, expected);
    }
}
```

#### slide-rs/core/src/flags.rs
```rust
use std::time::Duration;

use env_flags::env_flags;

env_flags! {
    pub OPENAI_API_BASE: &str = "https://api.openai.com/v1";

    /// Fallback when the provider-specific key is not set.
    pub OPENAI_API_KEY: Option<&str> = None;
    pub OPENAI_TIMEOUT_MS: Duration = Duration::from_millis(300_000), |value| {
        value.parse().map(Duration::from_millis)
    };

    /// Fixture path for offline tests (see client.rs).
    pub SLIDE_RS_SSE_FIXTURE: Option<&str> = None;
}
```

#### slide-rs/core/src/seatbelt.rs
```rust
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::process::Child;

use crate::protocol::SandboxPolicy;
use crate::spawn::SLIDE_SANDBOX_ENV_VAR;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;

const MACOS_SEATBELT_BASE_POLICY: &str = include_str!("seatbelt_base_policy.sbpl");

/// When working with `sandbox-exec`, only consider `sandbox-exec` in `/usr/bin`
/// to defend against an attacker trying to inject a malicious version on the
/// PATH. If /usr/bin/sandbox-exec has been tampered with, then the attacker
/// already has root access.
const MACOS_PATH_TO_SEATBELT_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

pub async fn spawn_command_under_seatbelt(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: PathBuf,
    stdio_policy: StdioPolicy,
    mut env: HashMap<String, String>,
) -> std::io::Result<Child> {
    let args = create_seatbelt_command_args(command, sandbox_policy, &cwd);
    let arg0 = None;
    env.insert(SLIDE_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
    spawn_child_async(
        PathBuf::from(MACOS_PATH_TO_SEATBELT_EXECUTABLE),
        args,
        arg0,
        cwd,
        sandbox_policy,
        stdio_policy,
        env,
    )
    .await
}

fn create_seatbelt_command_args(
    command: Vec<String>,
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> Vec<String> {
    let (file_write_policy, extra_cli_args) = {
        if sandbox_policy.has_full_disk_write_access() {
            // Allegedly, this is more permissive than `(allow file-write*)`.
            (
                r#"(allow file-write* (regex #"^/"))"#.to_string(),
                Vec::<String>::new(),
            )
        } else {
            let writable_roots = sandbox_policy.get_writable_roots_with_cwd(cwd);

            let mut writable_folder_policies: Vec<String> = Vec::new();
            let mut cli_args: Vec<String> = Vec::new();

            for (index, wr) in writable_roots.iter().enumerate() {
                // Canonicalize to avoid mismatches like /var vs /private/var on macOS.
                let canonical_root = wr.root.canonicalize().unwrap_or_else(|_| wr.root.clone());
                let root_param = format!("WRITABLE_ROOT_{index}");
                cli_args.push(format!(
                    "-D{root_param}={}",
                    canonical_root.to_string_lossy()
                ));

                if wr.read_only_subpaths.is_empty() {
                    writable_folder_policies.push(format!("(subpath (param \"{root_param}\"))"));
                } else {
                    // Add parameters for each read-only subpath and generate
                    // the `(require-not ...)` clauses.
                    let mut require_parts: Vec<String> = Vec::new();
                    require_parts.push(format!("(subpath (param \"{root_param}\"))"));
                    for (subpath_index, ro) in wr.read_only_subpaths.iter().enumerate() {
                        let canonical_ro = ro.canonicalize().unwrap_or_else(|_| ro.clone());
                        let ro_param = format!("WRITABLE_ROOT_{index}_RO_{subpath_index}");
                        cli_args.push(format!("-D{ro_param}={}", canonical_ro.to_string_lossy()));
                        require_parts
                            .push(format!("(require-not (subpath (param \"{ro_param}\")))"));
                    }
                    let policy_component = format!("(require-all {} )", require_parts.join(" "));
                    writable_folder_policies.push(policy_component);
                }
            }

            if writable_folder_policies.is_empty() {
                ("".to_string(), Vec::<String>::new())
            } else {
                let file_write_policy = format!(
                    "(allow file-write*\n{}\n)",
                    writable_folder_policies.join(" ")
                );
                (file_write_policy, cli_args)
            }
        }
    };

    let file_read_policy = if sandbox_policy.has_full_disk_read_access() {
        "; allow read-only file operations\n(allow file-read*)"
    } else {
        ""
    };

    // TODO(mbolin): apply_patch calls must also honor the SandboxPolicy.
    let network_policy = if sandbox_policy.has_full_network_access() {
        "(allow network-outbound)\n(allow network-inbound)\n(allow system-socket)"
    } else {
        ""
    };

    let full_policy = format!(
        "{MACOS_SEATBELT_BASE_POLICY}\n{file_read_policy}\n{file_write_policy}\n{network_policy}"
    );

    let mut seatbelt_args: Vec<String> = vec!["-p".to_string(), full_policy];
    seatbelt_args.extend(extra_cli_args);
    seatbelt_args.push("--".to_string());
    seatbelt_args.extend(command);
    seatbelt_args
}

#[cfg(test)]
mod tests {
    use super::MACOS_SEATBELT_BASE_POLICY;
    use super::create_seatbelt_command_args;
    use crate::protocol::SandboxPolicy;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn create_seatbelt_args_with_read_only_git_subpath() {
        if cfg!(target_os = "windows") {
            // /tmp does not exist on Windows, so skip this test.
            return;
        }

        // Create a temporary workspace with two writable roots: one containing
        // a top-level .git directory and one without it.
        let tmp = TempDir::new().expect("tempdir");
        let PopulatedTmp {
            root_with_git,
            root_without_git,
            root_with_git_canon,
            root_with_git_git_canon,
            root_without_git_canon,
        } = populate_tmpdir(tmp.path());
        let cwd = tmp.path().join("cwd");

        // Build a policy that only includes the two test roots as writable and
        // does not automatically include defaults TMPDIR or /tmp.
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![root_with_git.clone(), root_without_git.clone()],
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };

        let args = create_seatbelt_command_args(
            vec!["/bin/echo".to_string(), "hello".to_string()],
            &policy,
            &cwd,
        );

        // Build the expected policy text using a raw string for readability.
        // Note that the policy includes:
        // - the base policy,
        // - read-only access to the filesystem,
        // - write access to WRITABLE_ROOT_0 (but not its .git) and WRITABLE_ROOT_1.
        let expected_policy = format!(
            r#"{MACOS_SEATBELT_BASE_POLICY}
; allow read-only file operations
(allow file-read*)
(allow file-write*
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) ) (subpath (param "WRITABLE_ROOT_1")) (subpath (param "WRITABLE_ROOT_2"))
)
"#,
        );

        let mut expected_args = vec![
            "-p".to_string(),
            expected_policy,
            format!(
                "-DWRITABLE_ROOT_0={}",
                root_with_git_canon.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_0={}",
                root_with_git_git_canon.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_1={}",
                root_without_git_canon.to_string_lossy()
            ),
            format!("-DWRITABLE_ROOT_2={}", cwd.to_string_lossy()),
        ];

        expected_args.extend(vec![
            "--".to_string(),
            "/bin/echo".to_string(),
            "hello".to_string(),
        ]);

        assert_eq!(expected_args, args);
    }

    #[test]
    fn create_seatbelt_args_for_cwd_as_git_repo() {
        if cfg!(target_os = "windows") {
            // /tmp does not exist on Windows, so skip this test.
            return;
        }

        // Create a temporary workspace with two writable roots: one containing
        // a top-level .git directory and one without it.
        let tmp = TempDir::new().expect("tempdir");
        let PopulatedTmp {
            root_with_git,
            root_with_git_canon,
            root_with_git_git_canon,
            ..
        } = populate_tmpdir(tmp.path());

        // Build a policy that does not specify any writable_roots, but does
        // use the default ones (cwd and TMPDIR) and verifies the `.git` check
        // is done properly for cwd.
        let policy = SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        };

        let args = create_seatbelt_command_args(
            vec!["/bin/echo".to_string(), "hello".to_string()],
            &policy,
            root_with_git.as_path(),
        );

        let tmpdir_env_var = std::env::var("TMPDIR")
            .ok()
            .map(PathBuf::from)
            .and_then(|p| p.canonicalize().ok())
            .map(|p| p.to_string_lossy().to_string());

        let tempdir_policy_entry = if tmpdir_env_var.is_some() {
            r#" (subpath (param "WRITABLE_ROOT_2"))"#
        } else {
            ""
        };

        // Build the expected policy text using a raw string for readability.
        // Note that the policy includes:
        // - the base policy,
        // - read-only access to the filesystem,
        // - write access to WRITABLE_ROOT_0 (but not its .git) and WRITABLE_ROOT_1.
        let expected_policy = format!(
            r#"{MACOS_SEATBELT_BASE_POLICY}
; allow read-only file operations
(allow file-read*)
(allow file-write*
(require-all (subpath (param "WRITABLE_ROOT_0")) (require-not (subpath (param "WRITABLE_ROOT_0_RO_0"))) ) (subpath (param "WRITABLE_ROOT_1")){tempdir_policy_entry}
)
"#,
        );

        let mut expected_args = vec![
            "-p".to_string(),
            expected_policy,
            format!(
                "-DWRITABLE_ROOT_0={}",
                root_with_git_canon.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_0_RO_0={}",
                root_with_git_git_canon.to_string_lossy()
            ),
            format!(
                "-DWRITABLE_ROOT_1={}",
                PathBuf::from("/tmp")
                    .canonicalize()
                    .expect("canonicalize /tmp")
                    .to_string_lossy()
            ),
        ];

        if let Some(p) = tmpdir_env_var {
            expected_args.push(format!("-DWRITABLE_ROOT_2={p}"));
        }

        expected_args.extend(vec![
            "--".to_string(),
            "/bin/echo".to_string(),
            "hello".to_string(),
        ]);

        assert_eq!(expected_args, args);
    }

    struct PopulatedTmp {
        root_with_git: PathBuf,
        root_without_git: PathBuf,
        root_with_git_canon: PathBuf,
        root_with_git_git_canon: PathBuf,
        root_without_git_canon: PathBuf,
    }

    fn populate_tmpdir(tmp: &Path) -> PopulatedTmp {
        let root_with_git = tmp.join("with_git");
        let root_without_git = tmp.join("no_git");
        fs::create_dir_all(&root_with_git).expect("create with_git");
        fs::create_dir_all(&root_without_git).expect("create no_git");
        fs::create_dir_all(root_with_git.join(".git")).expect("create .git");

        // Ensure we have canonical paths for -D parameter matching.
        let root_with_git_canon = root_with_git.canonicalize().expect("canonicalize with_git");
        let root_with_git_git_canon = root_with_git_canon.join(".git");
        let root_without_git_canon = root_without_git
            .canonicalize()
            .expect("canonicalize no_git");
        PopulatedTmp {
            root_with_git,
            root_without_git,
            root_with_git_canon,
            root_with_git_git_canon,
            root_without_git_canon,
        }
    }
}
```

#### slide-rs/execpolicy/src/lib.rs
```rust
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#[macro_use]
extern crate starlark;

mod arg_matcher;
mod arg_resolver;
mod arg_type;
mod error;
mod exec_call;
mod execv_checker;
mod opt;
mod policy;
mod policy_parser;
mod program;
mod sed_command;
mod valid_exec;

pub use arg_matcher::ArgMatcher;
pub use arg_resolver::PositionalArg;
pub use arg_type::ArgType;
pub use error::Error;
pub use error::Result;
pub use exec_call::ExecCall;
pub use execv_checker::ExecvChecker;
pub use opt::Opt;
pub use policy::Policy;
pub use policy_parser::PolicyParser;
pub use program::Forbidden;
pub use program::MatchedExec;
pub use program::NegativeExamplePassedCheck;
pub use program::PositiveExampleFailedCheck;
pub use program::ProgramSpec;
pub use sed_command::parse_sed_command;
pub use valid_exec::MatchedArg;
pub use valid_exec::MatchedFlag;
pub use valid_exec::MatchedOpt;
pub use valid_exec::ValidExec;

const DEFAULT_POLICY: &str = include_str!("default.policy");

pub fn get_default_policy() -> starlark::Result<Policy> {
    let parser = PolicyParser::new("#default", DEFAULT_POLICY);
    parser.parse()
}
```

#### slide-rs/tui/src/lib.rs
```rust
// Forbid accidental stdout/stderr writes in the *library* portion of the TUI.
// The standalone `slide-tui` binary prints a short help message before the
// alternate‑screen mode starts; that file opts‑out locally via `allow`.
#![deny(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::disallowed_methods)]
use app::App;
use slide_core::BUILT_IN_OSS_MODEL_PROVIDER_ID;
use slide_core::config::Config;
use slide_core::config::ConfigOverrides;
use slide_core::config::ConfigToml;
use slide_core::config::find_slide_home;
use slide_core::config::load_config_as_toml_with_cli_overrides;
use slide_core::protocol::AskForApproval;
use slide_core::protocol::SandboxPolicy;
use slide_login::AuthManager;
use slide_login::AuthMode;
use slide_login::SlideAuth;
use slide_ollama::DEFAULT_OSS_MODEL;
use slide_protocol::config_types::SandboxMode;
use std::fs::OpenOptions;
use std::path::PathBuf;
use tracing::error;
use tracing_appender::non_blocking;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

mod app;
mod app_backtrack;
mod app_event;
mod app_event_sender;
mod backtrack_helpers;
mod bottom_pane;
mod chatwidget;
mod citation_regex;
mod cli;
mod clipboard_paste;
mod common;
pub mod custom_terminal;
mod diff_render;
mod exec_command;
mod file_search;
mod get_git_diff;
mod history_cell;
pub mod insert_history;
pub mod live_wrap;
mod markdown;
mod markdown_stream;
pub mod onboarding;
mod pager_overlay;
mod render;
mod session_log;
mod shimmer;
mod slash_command;
mod status_indicator_widget;
mod streaming;
mod text_formatting;
mod tui;
mod user_approval_widget;

// Internal vt100-based replay tests live as a separate source file to keep them
// close to the widget code. Include them in unit tests.
#[cfg(test)]
mod chatwidget_stream_tests;

#[cfg(not(debug_assertions))]
mod updates;

pub use cli::Cli;

use crate::onboarding::TrustDirectorySelection;
use crate::onboarding::onboarding_screen::OnboardingScreenArgs;
use crate::onboarding::onboarding_screen::run_onboarding_app;
use crate::tui::Tui;

// (tests access modules directly within the crate)

pub async fn run_main(
    cli: Cli,
    slide_linux_sandbox_exe: Option<PathBuf>,
) -> std::io::Result<slide_core::protocol::TokenUsage> {
    let (sandbox_mode, approval_policy) = if cli.full_auto {
        (
            Some(SandboxMode::WorkspaceWrite),
            Some(AskForApproval::OnFailure),
        )
    } else if cli.dangerously_bypass_approvals_and_sandbox {
        (
            Some(SandboxMode::DangerFullAccess),
            Some(AskForApproval::Never),
        )
    } else {
        (
            cli.sandbox_mode.map(Into::<SandboxMode>::into),
            cli.approval_policy.map(Into::into),
        )
    };

    // When using `--oss`, let the bootstrapper pick the model (defaulting to
    // gpt-oss:20b) and ensure it is present locally. Also, force the built‑in
    // `oss` model provider.
    let model = if let Some(model) = &cli.model {
        Some(model.clone())
    } else if cli.oss {
        Some(DEFAULT_OSS_MODEL.to_owned())
    } else {
        None // No model specified, will use the default.
    };

    let model_provider_override = if cli.oss {
        Some(BUILT_IN_OSS_MODEL_PROVIDER_ID.to_owned())
    } else {
        None
    };

    // canonicalize the cwd
    let cwd = cli.cwd.clone().map(|p| p.canonicalize().unwrap_or(p));

    let overrides = ConfigOverrides {
        model,
        approval_policy,
        sandbox_mode,
        cwd,
        model_provider: model_provider_override,
        config_profile: cli.config_profile.clone(),
        slide_linux_sandbox_exe,
        base_instructions: None,
        include_plan_tool: Some(true),
        include_apply_patch_tool: None,
        include_view_image_tool: None,
        disable_response_storage: cli.oss.then_some(true),
        show_raw_agent_reasoning: cli.oss.then_some(true),
        tools_web_search_request: cli.web_search.then_some(true),
    };
    let raw_overrides = cli.config_overrides.raw_overrides.clone();
    let overrides_cli = slide_common::CliConfigOverrides { raw_overrides };
    let cli_kv_overrides = match overrides_cli.parse_overrides() {
        Ok(v) => v,
        #[allow(clippy::print_stderr)]
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    let mut config = {
        // Load configuration and support CLI overrides.

        #[allow(clippy::print_stderr)]
        match Config::load_with_cli_overrides(cli_kv_overrides.clone(), overrides) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("Error loading configuration: {err}");
                std::process::exit(1);
            }
        }
    };

    // we load config.toml here to determine project state.
    #[allow(clippy::print_stderr)]
    let config_toml = {
        let slide_home = match find_slide_home() {
            Ok(slide_home) => slide_home,
            Err(err) => {
                eprintln!("Error finding slide home: {err}");
                std::process::exit(1);
            }
        };

        match load_config_as_toml_with_cli_overrides(&slide_home, cli_kv_overrides) {
            Ok(config_toml) => config_toml,
            Err(err) => {
                eprintln!("Error loading config.toml: {err}");
                std::process::exit(1);
            }
        }
    };

    let should_show_trust_screen = determine_repo_trust_state(
        &mut config,
        &config_toml,
        approval_policy,
        sandbox_mode,
        cli.config_profile.clone(),
    )?;

    let log_dir = slide_core::config::log_dir(&config)?;
    std::fs::create_dir_all(&log_dir)?;
    // Open (or create) your log file, appending to it.
    let mut log_file_opts = OpenOptions::new();
    log_file_opts.create(true).append(true);

    // Ensure the file is only readable and writable by the current user.
    // Doing the equivalent to `chmod 600` on Windows is quite a bit more code
    // and requires the Windows API crates, so we can reconsider that when
    // Slide CLI is officially supported on Windows.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        log_file_opts.mode(0o600);
    }

    let log_file = log_file_opts.open(log_dir.join("slide-tui.log"))?;

    // Wrap file in non‑blocking writer.
    let (non_blocking, _guard) = non_blocking(log_file);

    // use RUST_LOG env var, default to info for slide crates.
    let env_filter = || {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("slide_core=info,slide_tui=info"))
    };

    // Build layered subscriber:
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_target(false)
        .with_filter(env_filter());

    if cli.oss {
        slide_ollama::ensure_oss_ready(&config)
            .await
            .map_err(|e| std::io::Error::other(format!("OSS setup failed: {e}")))?;
    }

    let _ = tracing_subscriber::registry().with(file_layer).try_init();

    run_ratatui_app(cli, config, should_show_trust_screen)
        .await
        .map_err(|err| std::io::Error::other(err.to_string()))
}

async fn run_ratatui_app(
    cli: Cli,
    config: Config,
    should_show_trust_screen: bool,
) -> color_eyre::Result<slide_core::protocol::TokenUsage> {
    let mut config = config;
    color_eyre::install()?;

    // Forward panic reports through tracing so they appear in the UI status
    // line, but do not swallow the default/color-eyre panic handler.
    // Chain to the previous hook so users still get a rich panic report
    // (including backtraces) after we restore the terminal.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("panic: {info}");
        prev_hook(info);
    }));
    let mut terminal = tui::init()?;
    terminal.clear()?;

    let mut tui = Tui::new(terminal);

    // Show update banner in terminal history (instead of stderr) so it is visible
    // within the TUI scrollback. Building spans keeps styling consistent.
    #[cfg(not(debug_assertions))]
    if let Some(latest_version) = updates::get_upgrade_version(&config) {
        use ratatui::style::Stylize as _;
        use ratatui::text::Line;
        use ratatui::text::Span;

        let current_version = env!("CARGO_PKG_VERSION");
        let exe = std::env::current_exe()?;
        let managed_by_npm = std::env::var_os("SLIDE_MANAGED_BY_NPM").is_some();

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![
            "✨⬆️ Update available!".bold().cyan(),
            Span::raw(" "),
            Span::raw(format!("{current_version} -> {latest_version}.")),
        ]));

        if managed_by_npm {
            let npm_cmd = "npm install -g @openai/slide@latest";
            lines.push(Line::from(vec![
                Span::raw("Run "),
                npm_cmd.cyan(),
                Span::raw(" to update."),
            ]));
        } else if cfg!(target_os = "macos")
            && (exe.starts_with("/opt/homebrew") || exe.starts_with("/usr/local"))
        {
            let brew_cmd = "brew upgrade slide";
            lines.push(Line::from(vec![
                Span::raw("Run "),
                brew_cmd.cyan(),
                Span::raw(" to update."),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw("See "),
                "https://github.com/openai/slide/releases/latest".cyan(),
                Span::raw(" for the latest releases and installation options."),
            ]));
        }

        lines.push(Line::from(""));
        tui.insert_history_lines(lines);
    }

    // Initialize high-fidelity session event logging if enabled.
    session_log::maybe_init(&config);

    let Cli { prompt, images, .. } = cli;

    let auth_manager = AuthManager::shared(config.slide_home.clone(), config.preferred_auth_method);
    let login_status = get_login_status(&config);
    let should_show_onboarding =
        should_show_onboarding(login_status, &config, should_show_trust_screen);
    if should_show_onboarding {
        let directory_trust_decision = run_onboarding_app(
            OnboardingScreenArgs {
                slide_home: config.slide_home.clone(),
                cwd: config.cwd.clone(),
                show_login_screen: should_show_login_screen(login_status, &config),
                show_trust_screen: should_show_trust_screen,
                login_status,
                preferred_auth_method: config.preferred_auth_method,
                auth_manager: auth_manager.clone(),
            },
            &mut tui,
        )
        .await?;
        if let Some(TrustDirectorySelection::Trust) = directory_trust_decision {
            config.approval_policy = AskForApproval::OnRequest;
            config.sandbox_policy = SandboxPolicy::new_workspace_write_policy();
        }
    }

    let app_result = App::run(&mut tui, auth_manager, config, prompt, images).await;

    restore();
    // Mark the end of the recorded session.
    session_log::log_session_end();
    // ignore error when collecting usage – report underlying error instead
    app_result
}

#[expect(
    clippy::print_stderr,
    reason = "TUI should no longer be displayed, so we can write to stderr."
)]
fn restore() {
    if let Err(err) = tui::restore() {
        eprintln!(
            "failed to restore terminal. Run `reset` or restart your terminal to recover: {err}"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginStatus {
    AuthMode(AuthMode),
    NotAuthenticated,
}

fn get_login_status(config: &Config) -> LoginStatus {
    if config.model_provider.requires_openai_auth {
        // Reading the OpenAI API key is an async operation because it may need
        // to refresh the token. Block on it.
        let slide_home = config.slide_home.clone();
        match SlideAuth::from_slide_home(&slide_home, config.preferred_auth_method) {
            Ok(Some(auth)) => LoginStatus::AuthMode(auth.mode),
            Ok(None) => LoginStatus::NotAuthenticated,
            Err(err) => {
                error!("Failed to read auth.json: {err}");
                LoginStatus::NotAuthenticated
            }
        }
    } else {
        LoginStatus::NotAuthenticated
    }
}

/// Determine if user has configured a sandbox / approval policy,
/// or if the current cwd project is trusted, and updates the config
/// accordingly.
fn determine_repo_trust_state(
    config: &mut Config,
    config_toml: &ConfigToml,
    approval_policy_overide: Option<AskForApproval>,
    sandbox_mode_override: Option<SandboxMode>,
    config_profile_override: Option<String>,
) -> std::io::Result<bool> {
    let config_profile = config_toml.get_config_profile(config_profile_override)?;

    if approval_policy_overide.is_some() || sandbox_mode_override.is_some() {
        // if the user has overridden either approval policy or sandbox mode,
        // skip the trust flow
        Ok(false)
    } else if config_profile.approval_policy.is_some() {
        // if the user has specified settings in a config profile, skip the trust flow
        // todo: profile sandbox mode?
        Ok(false)
    } else if config_toml.approval_policy.is_some() || config_toml.sandbox_mode.is_some() {
        // if the user has specified either approval policy or sandbox mode in config.toml
        // skip the trust flow
        Ok(false)
    } else if config_toml.is_cwd_trusted(&config.cwd) {
        // if the current cwd project is trusted and no config has been set
        // skip the trust flow and set the approval policy and sandbox mode
        config.approval_policy = AskForApproval::OnRequest;
        config.sandbox_policy = SandboxPolicy::new_workspace_write_policy();
        Ok(false)
    } else {
        // if none of the above conditions are met, show the trust screen
        Ok(true)
    }
}

fn should_show_onboarding(
    login_status: LoginStatus,
    config: &Config,
    show_trust_screen: bool,
) -> bool {
    if show_trust_screen {
        return true;
    }

    should_show_login_screen(login_status, config)
}

fn should_show_login_screen(login_status: LoginStatus, config: &Config) -> bool {
    // Only show the login screen for providers that actually require OpenAI auth
    // (OpenAI or equivalents). For OSS/other providers, skip login entirely.
    if !config.model_provider.requires_openai_auth {
        return false;
    }

    match login_status {
        LoginStatus::NotAuthenticated => true,
        LoginStatus::AuthMode(method) => method != config.preferred_auth_method,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(preferred: AuthMode) -> Config {
        let mut cfg = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            std::env::temp_dir(),
        )
        .expect("load default config");
        cfg.preferred_auth_method = preferred;
        cfg
    }

    #[test]
    fn shows_login_when_not_authenticated() {
        let cfg = make_config(AuthMode::ChatGPT);
        assert!(should_show_login_screen(
            LoginStatus::NotAuthenticated,
            &cfg
        ));
    }

    #[test]
    fn shows_login_when_api_key_but_prefers_chatgpt() {
        let cfg = make_config(AuthMode::ChatGPT);
        assert!(should_show_login_screen(
            LoginStatus::AuthMode(AuthMode::ApiKey),
            &cfg
        ))
    }

    #[test]
    fn hides_login_when_api_key_and_prefers_api_key() {
        let cfg = make_config(AuthMode::ApiKey);
        assert!(!should_show_login_screen(
            LoginStatus::AuthMode(AuthMode::ApiKey),
            &cfg
        ))
    }

    #[test]
    fn hides_login_when_chatgpt_and_prefers_chatgpt() {
        let cfg = make_config(AuthMode::ChatGPT);
        assert!(!should_show_login_screen(
            LoginStatus::AuthMode(AuthMode::ChatGPT),
            &cfg
        ))
    }
}
```

#### slide-rs/protocol/src/lib.rs  
```rust
pub mod config_types;
pub mod custom_prompts;
pub mod mcp_protocol;
pub mod message_history;
pub mod models;
pub mod parse_command;
pub mod plan_tool;
pub mod protocol;
```

#### slide-rs/protocol/src/protocol.rs  
```rust
//! Defines the protocol for a Slide session between a client and an agent.
//!
//! Uses a SQ (Submission Queue) / EQ (Event Queue) pattern to asynchronously communicate
//! between user and agent.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use crate::custom_prompts::CustomPrompt;
use mcp_types::CallToolResult;
use mcp_types::Tool as McpTool;
use serde::Deserialize;
use serde::Serialize;
use serde_bytes::ByteBuf;
use strum_macros::Display;
use ts_rs::TS;
use uuid::Uuid;

use crate::config_types::ReasoningEffort as ReasoningEffortConfig;
use crate::config_types::ReasoningSummary as ReasoningSummaryConfig;
use crate::message_history::HistoryEntry;
use crate::models::ResponseItem;
use crate::parse_command::ParsedCommand;
use crate::plan_tool::UpdatePlanArgs;

/// Submission Queue Entry - requests from user
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Submission {
    /// Unique id for this Submission to correlate with Events
    pub id: String,
    /// Payload
    pub op: Op,
}

/// Submission operation
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum Op {
    /// Abort current task.
    /// This server sends [`EventMsg::TurnAborted`] in response.
    Interrupt,

    /// Input from the user
    UserInput {
        /// User input items, see `InputItem`
        items: Vec<InputItem>,
    },

    /// Similar to [`Op::UserInput`], but contains additional context required
    /// for a turn of a [`crate::slide_conversation::SlideConversation`].
    UserTurn {
        /// User input items, see `InputItem`
        items: Vec<InputItem>,

        /// `cwd` to use with the [`SandboxPolicy`] and potentially tool calls
        /// such as `local_shell`.
        cwd: PathBuf,

        /// Policy to use for command approval.
        approval_policy: AskForApproval,

        /// Policy to use for tool calls such as `local_shell`.
        sandbox_policy: SandboxPolicy,

        /// Must be a valid model slug for the [`crate::client::ModelClient`]
        /// associated with this conversation.
        model: String,

        /// Will only be honored if the model is configured to use reasoning.
        effort: ReasoningEffortConfig,

        /// Will only be honored if the model is configured to use reasoning.
        summary: ReasoningSummaryConfig,
    },

    /// Override parts of the persistent turn context for subsequent turns.
    ///
    /// All fields are optional; when omitted, the existing value is preserved.
    /// This does not enqueue any input – it only updates defaults used for
    /// future `UserInput` turns.
    OverrideTurnContext {
        /// Updated `cwd` for sandbox/tool calls.
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<PathBuf>,

        /// Updated command approval policy.
        #[serde(skip_serializing_if = "Option::is_none")]
        approval_policy: Option<AskForApproval>,

        /// Updated sandbox policy for tool calls.
        #[serde(skip_serializing_if = "Option::is_none")]
        sandbox_policy: Option<SandboxPolicy>,

        /// Updated model slug. When set, the model family is derived
        /// automatically from the new model name.
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,

        /// Updated effort level for reasoning (when supported by the model).
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning_effort: Option<ReasoningEffortConfig>,

        /// Updated summary preference for reasoning (when supported by the model).
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning_summary: Option<ReasoningSummaryConfig>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputItem {
    /// Plain text submitted by the user.
    Text { text: String },

    /// Image submitted by the user. The URL can be a data URL or a file:// URL.
    Image { image_url: String },

    /// A custom prompt loaded from ~/.slide/prompts.
    CustomPrompt(CustomPrompt),

    /// Results from a parsed command like `/file` or `/url`.
    ParsedCommand(ParsedCommand),
}

/// Event Queue Entry - responses from an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    #[serde(flatten)]
    pub msg: EventMsg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventMsg {
    /// Sent after a [`Submission`] is received and will be processed.
    Created,

    /// Sent when agent execution begins for the current turn.
    TurnStarted,

    /// A function/tool call is being prepared for execution.
    FunctionCallQueued {
        call_id: String,
        function_name: String,
    },

    /// Sent after calling exec tool to track streaming output.
    ExecCommandOutputDelta(ExecCommandOutputDeltaEvent),

    /// A chunk of response content from the model. Sent repeatedly during
    /// agent turns with incremental deltas.
    ResponseDelta { delta: String },

    /// Agent reasoning content delta (when reasoning mode is enabled).
    AgentReasoningDelta { delta: String },

    /// Agent reasoning summary delta (when reasoning summaries are enabled).
    AgentReasoningSummaryDelta { delta: String },

    /// A complete response item from the model.
    ResponseItem(ResponseItem),

    /// The current turn has finished successfully.
    TurnCompleted { token_usage: Option<TokenUsage> },

    /// The current turn was aborted due to an interrupt signal.
    TurnAborted,

    /// The current turn failed due to an error.
    TurnError { error: String },

    /// Update to the plan tool showing current task status and steps.
    PlanUpdate(UpdatePlanArgs),

    /// Request user approval for a command execution.
    UserApprovalRequired {
        request_id: String,
        command: Vec<String>,
        justification: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecCommandOutputDeltaEvent {
    pub call_id: String,
    pub stream: ExecOutputStream,
    pub chunk: ByteBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecOutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl TokenUsage {
    pub fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            input_tokens,
            output_tokens,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.input_tokens == 0 && self.output_tokens == 0
    }

    pub fn total(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}

impl std::ops::Add for TokenUsage {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            input_tokens: self.input_tokens + rhs.input_tokens,
            output_tokens: self.output_tokens + rhs.output_tokens,
        }
    }
}

impl std::ops::AddAssign for TokenUsage {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalOutput {
    pub token_usage: TokenUsage,
}

impl From<TokenUsage> for FinalOutput {
    fn from(token_usage: TokenUsage) -> Self {
        Self { token_usage }
    }
}

impl fmt::Display for FinalOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        serde_json::to_string(self)
            .map_err(|_| fmt::Error)?
            .fmt(f)
    }
}

/// Policy for when to ask for user approval before executing commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskForApproval {
    /// Always ask for approval before executing any command.
    Always,
    /// Ask for approval when executing commands but allow auto-approval for certain safe commands.
    OnRequest,
    /// Only ask for approval when a command fails or produces unexpected results.
    OnFailure,
    /// Only ask for approval for commands that are not trusted.
    UnlessTrusted,
    /// Never ask for approval (dangerous).
    Never,
}

impl Default for AskForApproval {
    fn default() -> Self {
        Self::UnlessTrusted
    }
}

impl FromStr for AskForApproval {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "always" => Ok(Self::Always),
            "on-request" => Ok(Self::OnRequest),
            "on-failure" => Ok(Self::OnFailure),
            "unless-trusted" => Ok(Self::UnlessTrusted),
            "never" => Ok(Self::Never),
            _ => Err(format!("Invalid approval policy: {s}")),
        }
    }
}

/// Sandbox policy defining what file system operations are allowed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SandboxPolicy {
    /// Read-only access to the entire filesystem.
    ReadOnly,

    /// Read-only access with write access to specific workspace roots.
    WorkspaceWrite {
        writable_roots: Vec<PathBuf>,
        network_access: bool,
        exclude_tmpdir_env_var: bool,
        exclude_slash_tmp: bool,
    },

    /// Full access to everything (dangerous).
    DangerFullAccess,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self::ReadOnly
    }
}

impl SandboxPolicy {
    pub fn new_read_only_policy() -> Self {
        Self::ReadOnly
    }

    pub fn new_workspace_write_policy() -> Self {
        Self::WorkspaceWrite {
            writable_roots: vec![],
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }
    }

    pub fn has_full_disk_read_access(&self) -> bool {
        match self {
            Self::ReadOnly | Self::WorkspaceWrite { .. } => true,
            Self::DangerFullAccess => true,
        }
    }

    pub fn has_full_disk_write_access(&self) -> bool {
        matches!(self, Self::DangerFullAccess)
    }

    pub fn has_full_network_access(&self) -> bool {
        match self {
            Self::ReadOnly => false,
            Self::WorkspaceWrite { network_access, .. } => *network_access,
            Self::DangerFullAccess => true,
        }
    }

    pub fn get_writable_roots_with_cwd(&self, cwd: &Path) -> Vec<WritableRoot> {
        match self {
            Self::ReadOnly => vec![],
            Self::WorkspaceWrite {
                writable_roots,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
                ..
            } => {
                let mut roots = Vec::new();

                // Add user-specified roots
                for root in writable_roots {
                    let read_only_subpaths = if root.join(".git").exists() {
                        vec![root.join(".git")]
                    } else {
                        vec![]
                    };
                    roots.push(WritableRoot {
                        root: root.clone(),
                        read_only_subpaths,
                    });
                }

                // Add current working directory (cwd)
                let cwd_read_only_subpaths = if cwd.join(".git").exists() {
                    vec![cwd.join(".git")]
                } else {
                    vec![]
                };
                roots.push(WritableRoot {
                    root: cwd.to_path_buf(),
                    read_only_subpaths: cwd_read_only_subpaths,
                });

                // Add system temp directories if not excluded
                if !exclude_slash_tmp {
                    roots.push(WritableRoot {
                        root: PathBuf::from("/tmp"),
                        read_only_subpaths: vec![],
                    });
                }

                if !exclude_tmpdir_env_var {
                    if let Ok(tmpdir) = std::env::var("TMPDIR") {
                        roots.push(WritableRoot {
                            root: PathBuf::from(tmpdir),
                            read_only_subpaths: vec![],
                        });
                    }
                }

                roots
            }
            Self::DangerFullAccess => vec![WritableRoot {
                root: PathBuf::from("/"),
                read_only_subpaths: vec![],
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WritableRoot {
    pub root: PathBuf,
    pub read_only_subpaths: Vec<PathBuf>,
}
```

#### slide-rs/tui/src/main.rs
```rust
use clap::Parser;
use slide_arg0::arg0_dispatch_or_else;
use slide_common::CliConfigOverrides;
use slide_tui::Cli;
use slide_tui::run_main;

#[derive(Parser, Debug)]
struct TopCli {
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    inner: Cli,
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|slide_linux_sandbox_exe| async move {
        let top_cli = TopCli::parse();
        let mut inner = top_cli.inner;
        inner
            .config_overrides
            .raw_overrides
            .splice(0..0, top_cli.config_overrides.raw_overrides);
        let usage = run_main(inner, slide_linux_sandbox_exe).await?;
        if !usage.is_zero() {
            println!("{}", slide_core::protocol::FinalOutput::from(usage));
        }
        Ok(())
    })
}
```

#### slide-rs/core/src/util.rs
```rust
use std::path::Path;
use std::time::Duration;

use rand::Rng;

const INITIAL_DELAY_MS: u64 = 200;
const BACKOFF_FACTOR: f64 = 2.0;

pub(crate) fn backoff(attempt: u64) -> Duration {
    let exp = BACKOFF_FACTOR.powi(attempt.saturating_sub(1) as i32);
    let base = (INITIAL_DELAY_MS as f64 * exp) as u64;
    let jitter = rand::rng().random_range(0.9..1.1);
    Duration::from_millis((base as f64 * jitter) as u64)
}

/// Return `true` if the project folder specified by the `Config` is inside a
/// Git repository.
///
/// The check walks up the directory hierarchy looking for a `.git` file or
/// directory (note `.git` can be a file that contains a `gitdir` entry). This
/// approach does **not** require the `git` binary or the `git2` crate and is
/// therefore fairly lightweight.
///
/// Note that this does **not** detect *work‑trees* created with
/// `git worktree add` where the checkout lives outside the main repository
/// directory. If you need Slide to work from such a checkout simply pass the
/// `--allow-no-git-exec` CLI flag that disables the repo requirement.
pub fn is_inside_git_repo(base_dir: &Path) -> bool {
    let mut dir = base_dir.to_path_buf();

    loop {
        if dir.join(".git").exists() {
            return true;
        }

        // Pop one component (go up one directory).  `pop` returns false when
        // we have reached the filesystem root.
        if !dir.pop() {
            break;
        }
    }

    false
}
```
