# Codex CLI要件定義書

## プロジェクト概要

**Codex CLI**は、OpenAIが開発するローカル実行型のコーディングエージェントです。ユーザーは`npm install -g @openai/codex`コマンドでグローバルインストールし、`codex`コマンドで起動できます。

### 基本情報
- **パッケージ名**: `@openai/codex`
- **バージョン**: `0.0.0-dev`
- **ライセンス**: Apache-2.0
- **リポジトリ**: https://github.com/openai/codex.git

### 主要機能
- インタラクティブなコーディング支援
- 非インタラクティブなコマンド実行
- MCPサーバーモード
- 認証管理（ログイン/ログアウト）
- パッチ適用機能
- シェル補完スクリプト生成

## アーキテクチャ概要

Codex CLIは**ハイブリッドアーキテクチャ**を採用しています：

1. **Node.jsラッパー層**: NPMパッケージとして配布され、プラットフォーム検出とバイナリ起動を担当
2. **Rustコア層**: 実際のCLI機能をRustで実装、高速なネイティブバイナリとして動作

### 動作フロー

```
npm install -g @openai/codex
         ↓
package.json の bin エントリ
         ↓
bin/codex.js が実行
         ↓
プラットフォーム検出 (Darwin/Linux/Windows)
         ↓
対応するRustバイナリを呼び出し
         ↓ 
codex-{target-triple} を実行
         ↓
Rustメイン関数 (src/main.rs) が処理
```

## 関連ファイル詳細

### 1. NPMパッケージ定義 (`codex-cli/package.json`)

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

**ポイント**:
- `bin.codex`でエントリーポイントを指定
- ES Module形式 (`type: "module"`)
- Node.js 20以上が必要
- ripgrepを依存関係に含む

### 2. JavaScriptエントリーポイント (`codex-cli/bin/codex.js`)

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

**主要機能**:
- プラットフォーム/アーキテクチャ自動検出
- 対応するRustバイナリの動的実行
- ripgrepパスの解決とPATH更新
- シグナル転送（Ctrl-C等）
- 非同期プロセス管理

### 3. Rust CLI設定 (`codex-rs/cli/Cargo.toml`)

```toml
[package]
edition = "2024"
name = "codex-cli"
version = { workspace = true }

[[bin]]
name = "codex"
path = "src/main.rs"

[lib]
name = "codex_cli"
path = "src/lib.rs"

[lints]
workspace = true

[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
clap_complete = "4"
codex-arg0 = { path = "../arg0" }
codex-chatgpt = { path = "../chatgpt" }
codex-common = { path = "../common", features = ["cli"] }
codex-core = { path = "../core" }
codex-exec = { path = "../exec" }
codex-login = { path = "../login" }
codex-mcp-server = { path = "../mcp-server" }
codex-protocol = { path = "../protocol" }
codex-tui = { path = "../tui" }
serde_json = "1"
tokio = { version = "1", features = [
    "io-std",
    "macros",
    "process",
    "rt-multi-thread",
    "signal",
] }
tracing = "0.1.41"
tracing-subscriber = "0.3.19"
codex-protocol-ts = { path = "../protocol-ts" }
```

**特徴**:
- Rust 2024エディション
- clapでCLIパーサー実装
- 非同期ランタイムとしてTokio使用
- 複数のワークスペースクレートに依存

### 4. Rustメイン関数 (`codex-rs/cli/src/main.rs`) - 抜粋

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
```

**主要サブコマンド**:
- `exec` (`e`) - 非インタラクティブ実行
- `login` - 認証管理
- `logout` - ログアウト
- `mcp` - MCPサーバーモード
- `proto` (`p`) - プロトコルストリーム
- `completion` - シェル補完生成
- `apply` (`a`) - パッチ適用

### 5. Rustワークスペース設定 (`codex-rs/Cargo.toml`)

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
    "login",
    "mcp-client",
    "mcp-server",
    "mcp-types",
    "ollama",
    "protocol",
    "protocol-ts",
    "tui",
]
resolver = "2"

[workspace.package]
version = "0.0.0"
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
```

**最適化設定**:
- Link Time Optimization (LTO) - "fat"
- シンボルストリッピング
- 単一codegen-unit（バイナリサイズ最小化）

## インストール・実行プロセス

### 1. インストールフェーズ

```bash
npm install -g @openai/codex
```

1. NPMがpackage.jsonから`@openai/codex`パッケージをダウンロード
2. `files`配列に指定された`bin`と`dist`ディレクトリがインストール
3. `bin.codex`エントリにより、`codex`コマンドが`bin/codex.js`にリンク
4. グローバルPATHに`codex`コマンドが追加

### 2. 実行フェーズ

```bash
codex [subcommand] [options]
```

1. **JavaScript層**: `bin/codex.js`が起動
   - プラットフォーム検出 (Darwin, Linux, Windows)
   - アーキテクチャ検出 (x64, arm64)
   - 対応するバイナリパス構築: `codex-{target-triple}`

2. **バイナリ起動**: 
   - ripgrepパスをPATHに追加
   - `CODEX_MANAGED_BY_NPM=1`環境変数設定
   - 非同期でRustバイナリを起動

3. **Rust層**: `main.rs`でCLIパース
   - clapによるコマンドライン解析
   - サブコマンドに応じた処理分岐
   - TUI/非インタラクティブモード実行

### 3. シグナル処理

- SIGINT (Ctrl-C)、SIGTERM、SIGHUPを子プロセスに転送
- 子プロセスの終了ステータスを親に伝播
- グレースフル・シャットダウン保証

## テクニカル要件

### 必要環境
- **Node.js**: >=20
- **対応プラットフォーム**:
  - macOS (x64, arm64)
  - Linux (x64, arm64, musl)
  - Windows (x64)

### 依存関係
- **JavaScript**: @vscode/ripgrep
- **Rust**: 21の内部クレート + 外部クレート (clap, tokio, anyhow等)

### パフォーマンス要件
- バイナリサイズ最小化 (LTO, symbol stripping)
- 高速起動 (ネイティブバイナリ)
- 非同期I/O (Tokio)

## セキュリティ考慮事項

### サンドボックス機能
- **macOS**: Seatbelt サンドボックス
- **Linux**: Landlock + seccomp

### 認証管理
- ローカル認証情報ストレージ
- ChatGPT/APIキー連携
- セキュアログアウト機能

## 拡張性

### MCPサーバー機能
- Model Context Protocol対応
- 外部ツール連携インターフェース

### プロトコル拡張
- TypeScript型定義自動生成
- カスタムプロトコル対応

この要件定義書は、Codex CLIの完全な実装仕様を提供し、npm installから実行まで全プロセスを網羅しています。