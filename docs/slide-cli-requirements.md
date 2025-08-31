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

## 想定ディレクトリ構造（MVP）
```
repo-root/
├─ codex-cli/                # 既存: Codex 用 Node ランチャ
│  └─ ...
├─ codex-rs/                 # 既存: Rust ワークスペース
│  ├─ cli/                   # 既存: Rust CLI 本体
│  ├─ core/                  # 既存: コア機能
│  ├─ ...
│  └─ (他の既存クレート)
├─ slide-cli/                # 新規: Slide 用 Node ランチャ（codex-cli と同構成）
│  ├─ package.json
│  ├─ bin/
│  │  └─ slide.js           # エントリポイント（プラットフォーム判定→ slide-<target> 実行）
│  ├─ scripts/               # （必要なら）リリース・サンドボックス補助
│  └─ (bin/slide-<target> はリリース時に同梱)
├─ docs/
│  ├─ slide-cli-requirements.md
│  └─ ...
├─ slides/                   # 生成物（Markdown）出力先（初回生成時に作成）
│  └─ <timestamp>_<slug>.md
└─ ...
```

- Node ランチャを分ける理由: コマンド名 `slide` を npm 経由で配布しやすくするため（`codex` と並存）。
- Rust 側は既存 CLI にモード追加（`--app slide` or argv0）で最小改修方針。
- 将来、共通化できる箇所（スクリプト・配布手順）は `codex-cli` と揃える。

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
