# Codex CLI npm グローバルインストール完全アーキテクチャ

## 概要 / 目的

本文書は、`npm install -g @openai/codex` でCLIが動作する仕組みと、関係する実装ファイル一式を完全に文書化するものです。

## 仕組み

1. **実行名登録**: npm が `bin` マップで `codex` をPATHに登録
2. **JSエントリ**: `codex-cli/bin/codex.js` がOS/CPUでネイティブ実行ファイル名を決定
3. **ネイティブ起動**: 同梱された `bin/codex-<targetTriple>` を spawn で起動
4. **本体実装**: ネイティブは Rust の `codex-rs/cli`（バイナリ名 `codex`）
5. **同梱処理**: スクリプトが GitHub Actions の成果物を解凍して `codex-cli/bin/` に配置、npm 発行物に含める

## キーとなるファイル

### エントリ/パッケージ

#### `codex-cli/package.json` - パッケージ名・bin マップ・発行物定義

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

**重要ポイント**:
- `bin.codex` で実行名とエントリーポイントを定義
- `files` 配列で npm パッケージに含める資産を指定
- `@vscode/ripgrep` をランタイム依存に含む

#### `codex-cli/bin/codex.js` - OS/CPU検出とネイティブ起動

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

**核心機能**:
- プラットフォーム・アーキテクチャ自動検出
- 対応するRustバイナリパス構築
- ripgrepパスの動的解決とPATH更新  
- 非同期子プロセス起動と終了状態の伝播
- シグナル転送（SIGINT, SIGTERM, SIGHUP）

### ネイティブ本体（Rust CLI）

#### `codex-rs/cli/Cargo.toml` - Rustバイナリ定義

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

**注目点**:
- バイナリ名 `codex` でヘルプ表示名を統一
- clap でCLIパーサー実装
- Tokio非同期ランタイム

#### `codex-rs/cli/src/main.rs` - 全サブコマンド実装

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

// ... 他の構造体定義 ...

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
        // ... 他のサブコマンド処理 ...
    }
    Ok(())
}
```

**実装されるサブコマンド**:
- **デフォルト**: インタラクティブTUI
- `exec` (`e`): 非インタラクティブ実行
- `login`: 認証管理 
- `logout`: ログアウト
- `mcp`: MCPサーバーモード
- `proto` (`p`): プロトコルストリーム
- `completion`: シェル補完生成
- `debug`: デバッグ機能（Seatbelt/Landlock）
- `apply` (`a`): パッチ適用

### リリース/同梱スクリプト（npm パッケージにバイナリを入れる部分）

#### `codex-cli/scripts/install_native_deps.sh` - 各プラットフォーム用バイナリ解凍・配置

```bash
#!/usr/bin/env bash

# Install native runtime dependencies for codex-cli.
#
# Usage
#   install_native_deps.sh [--workflow-url URL] [CODEX_CLI_ROOT]

set -euo pipefail

# Until we start publishing stable GitHub releases, we have to grab the binaries
# from the GitHub Action that created them. Update the URL below to point to the
# appropriate workflow run:
WORKFLOW_URL="https://github.com/openai/codex/actions/runs/16840150768" # rust-v0.20.0-alpha.2

# Parse arguments and determine BIN_DIR
# ... (パース処理)

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

**機能**:
- GitHub Actions artifactsからzstd圧縮バイナリをダウンロード
- 各プラットフォーム向けに展開し`bin/codex-{target-triple}`として配置
- `gh` CLI必須（認証済み）

#### `codex-cli/scripts/stage_release.sh` - npm 発行用ステージング

```bash
#!/usr/bin/env bash
# Stages an npm release for @openai/codex.

set -euo pipefail

# Default to a timestamp-based version (keep same scheme as before)
VERSION="$(printf '0.1.%d' "$(date +%y%m%d%H%M)")"

# ... (フラグパース処理)

echo "Staging release in $TMPDIR"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CODEX_CLI_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

pushd "$CODEX_CLI_ROOT" >/dev/null

# 1. Build the JS artifacts ---------------------------------------------------

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

# Print final hint for convenience
echo "Next:  cd \"$TMPDIR\" && npm publish"
```

**ワークフロー**:
1. JSアーティファクト（`bin/codex.js`）をコピー
2. `package.json`のバージョン更新
3. `install_native_deps.sh`でネイティブバイナリ同梱
4. npm publish用ディレクトリ完成

#### `codex-cli/scripts/stage_rust_release.py` - リリースステージングヘルパー

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

**自動化**:
- GitHub ReleasesのブランチからRustワークフローを特定
- 対応するworkflow URLを`stage_release.sh`に渡す
- バージョンとSHA情報を自動取得

### CI/Release（参考）

#### `.github/workflows/ci.yml` - CI で stage_release.sh を実行して検証

```yaml
name: ci

on:
  pull_request: { branches: [main] }
  push: { branches: [main] }

jobs:
  build-test:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    env:
      NODE_OPTIONS: --max-old-space-size=4096
    steps:
      - name: Checkout repository
        uses: actions/checkout@v5

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22

      - name: Setup pnpm
        uses: pnpm/action-setup@v4
        with:
          version: 10.8.1
          run_install: false

      - name: Get pnpm store directory
        id: pnpm-cache
        shell: bash
        run: |
          echo "store_path=$(pnpm store path --silent)" >> $GITHUB_OUTPUT

      - name: Setup pnpm cache
        uses: actions/cache@v4
        with:
          path: ${{ steps.pnpm-cache.outputs.store_path }}
          key: ${{ runner.os }}-pnpm-store-${{ hashFiles('**/pnpm-lock.yaml') }}
          restore-keys: |
            ${{ runner.os }}-pnpm-store-

      - name: Install dependencies
        run: pnpm install

      # Run all tasks using workspace filters

      - name: Ensure staging a release works.
        env:
          GH_TOKEN: ${{ github.token }}
        run: ./codex-cli/scripts/stage_release.sh
```

**検証内容**:
- stage_release.shが正常動作することを確認
- npm package構成の妥当性を検証

#### `.github/workflows/rust-release.yml` - リリース時にnpm用ステージングを実施

```yaml
# Release workflow for codex-rs.
# To release, follow a workflow like:
# ```
# git tag -a rust-v0.1.0 -m "Release 0.1.0"
# git push origin rust-v0.1.0
# ```

name: rust-release
on:
  push:
    tags:
      - "rust-v*.*.*"

concurrency:
  group: ${{ github.workflow }}
  cancel-in-progress: true

jobs:
  tag-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5

      - name: Validate tag matches Cargo.toml version
        shell: bash
        run: |
          set -euo pipefail
          echo "::group::Tag validation"

          # 1. Must be a tag and match the regex
          [[ "${GITHUB_REF_TYPE}" == "tag" ]] \
            || { echo "❌  Not a tag push"; exit 1; }
          [[ "${GITHUB_REF_NAME}" =~ ^rust-v[0-9]+\.[0-9]+\.[0-9]+(-(alpha|beta)(\.[0-9]+)?)?$ ]] \
            || { echo "❌  Tag '${GITHUB_REF_NAME}' doesn't match expected format"; exit 1; }

          # 2. Extract versions
          tag_ver="${GITHUB_REF_NAME#rust-v}"
          cargo_ver="$(grep -m1 '^version' codex-rs/Cargo.toml \
                        | sed -E 's/version *= *"([^"]+)".*/\1/')"

          # 3. Compare
          [[ "${tag_ver}" == "${cargo_ver}" ]] \
            || { echo "❌  Tag ${tag_ver} ≠ Cargo.toml ${cargo_ver}"; exit 1; }

          echo "✅  Tag and Cargo.toml agree (${tag_ver})"
          echo "::endgroup::"

  build:
    needs: tag-check
    name: ${{ matrix.runner }} - ${{ matrix.target }}
    runs-on: ${{ matrix.runner }}
    timeout-minutes: 30
    defaults:
      run:
        working-directory: codex-rs

    strategy:
      fail-fast: false
      matrix:
        include:
          - runner: macos-14
            target: aarch64-apple-darwin
          - runner: macos-14
            target: x86_64-apple-darwin
          - runner: ubuntu-24.04
            target: x86_64-unknown-linux-musl
          - runner: ubuntu-24.04
            target: x86_64-unknown-linux-gnu
          - runner: ubuntu-24.04-arm
            target: aarch64-unknown-linux-musl
          - runner: ubuntu-24.04-arm
            target: aarch64-unknown-linux-gnu
          - runner: windows-latest
            target: x86_64-pc-windows-msvc

    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@1.89
        with:
          targets: ${{ matrix.target }}

      - uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            ${{ github.workspace }}/codex-rs/target/
          key: cargo-${{ matrix.runner }}-${{ matrix.target }}-release-${{ hashFiles('**/Cargo.lock') }}

      - if: ${{ matrix.target == 'x86_64-unknown-linux-musl' || matrix.target == 'aarch64-unknown-linux-musl'}}
        name: Install musl build tools
        run: |
          sudo apt install -y musl-tools pkg-config

      - name: Cargo build
        run: cargo build --target ${{ matrix.target }} --release --bin codex

      - name: Stage artifacts
        run: |
          mkdir -p artifact
          if [[ "${{ matrix.target }}" == *"windows"* ]]; then
            binary_name="codex-${{ matrix.target }}.exe"
            cp target/${{ matrix.target }}/release/codex.exe artifact/"${binary_name}"
          else
            binary_name="codex-${{ matrix.target }}"
            cp target/${{ matrix.target }}/release/codex artifact/"${binary_name}"
          fi

      - name: Compress binaries  
        run: |
          cd artifact
          zstd --ultra -22 -o codex-${{ matrix.target }}*.zst codex-${{ matrix.target }}*
          rm codex-${{ matrix.target }}  # Remove uncompressed version

      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.target }}
          path: codex-rs/artifact/
```

**ビルドマトリックス**:
- macOS: aarch64, x86_64 (Apple Silicon & Intel)
- Linux: x86_64, aarch64 (musl & gnu)
- Windows: x86_64 (MSVC)

**成果物処理**:
1. 各プラットフォームでcargo buildを実行
2. バイナリをzstdで圧縮（最高レベル `-22`）
3. GitHub Actions artifactsにアップロード

## 全体フロー詳細

### インストールフロー

```
npm install -g @openai/codex
         ↓
1. package.json 読み込み
   - name: "@openai/codex"
   - bin: { "codex": "bin/codex.js" }
   - files: ["bin", "dist"]
         ↓
2. npm がグローバルPATHに "codex" コマンド追加
   - symlink: /usr/local/bin/codex → .../lib/node_modules/@openai/codex/bin/codex.js
         ↓
3. ripgrep 依存関係インストール
   - @vscode/ripgrep がnode_modulesに配置
```

### 実行フロー

```
codex [args]
     ↓
1. bin/codex.js 起動
   - Node.js ESM モード
   - process.platform/arch 検出
     ↓
2. targetTriple 決定
   - darwin + arm64 → "aarch64-apple-darwin" 
   - linux + x64 → "x86_64-unknown-linux-musl"
   - win32 + x64 → "x86_64-pc-windows-msvc.exe"
     ↓
3. binaryPath 構築
   - "../bin/codex-{targetTriple}"
     ↓
4. ripgrep パス解決
   - @vscode/ripgrep から rgPath 取得
   - PATH 環境変数に追加
     ↓
5. spawn でネイティブバイナリ起動
   - env: { CODEX_MANAGED_BY_NPM: "1" }
   - stdio: "inherit" (パイプスルー)
   - シグナル転送設定
     ↓
6. Rust codex バイナリ実行
   - clap でCLI パース
   - サブコマンド振り分け
   - TUI/exec/login/mcp 等の実行
     ↓
7. 終了処理
   - 子プロセスの終了コード/シグナルを親に伝播
   - グレースフル・シャットダウン
```

### リリースフロー

```
Git タグ作成 (rust-v0.1.0)
         ↓
1. rust-release.yml トリガー
   - tag-check: Cargo.toml バージョン検証
         ↓
2. マルチプラットフォームビルド
   - macOS (aarch64, x86_64)
   - Linux (x86_64, aarch64) × (musl, gnu)  
   - Windows (x86_64)
         ↓
3. バイナリ圧縮・アップロード
   - zstd --ultra -22 圧縮
   - GitHub Actions artifacts保存
         ↓
4. npm パッケージステージング（手動）
   - stage_rust_release.py 実行
   - GitHub CLI で artifacts ダウンロード
   - install_native_deps.sh でバイナリ展開
   - stage_release.sh で発行用パッケージ作成
         ↓
5. npm publish（手動）
   - ステージングディレクトリから発行
   - 全プラットフォーム用バイナリ同梱
```

## 補足

### 公開済み npm パッケージの構成

```
@openai/codex/
├── package.json
├── README.md
├── bin/
│   ├── codex.js                              # Node.js エントリーポイント
│   ├── codex-aarch64-apple-darwin           # ARM64 macOS
│   ├── codex-x86_64-apple-darwin            # x64 macOS
│   ├── codex-aarch64-unknown-linux-musl     # ARM64 Linux
│   ├── codex-x86_64-unknown-linux-musl      # x64 Linux
│   └── codex-x86_64-pc-windows-msvc.exe     # x64 Windows
└── node_modules/
    └── @vscode/ripgrep/                      # ripgrep バイナリ
```

### プラットフォーム対応状況

| OS      | アーキテクチャ | Target Triple                | 対応状況 |
|---------|-------------|----------------------------|---------|
| macOS   | ARM64       | aarch64-apple-darwin        | ✅       |
| macOS   | x64         | x86_64-apple-darwin         | ✅       |
| Linux   | ARM64       | aarch64-unknown-linux-musl  | ✅       |
| Linux   | x64         | x86_64-unknown-linux-musl   | ✅       |
| Windows | x64         | x86_64-pc-windows-msvc.exe  | ✅       |
| Windows | ARM64       | (未対応)                     | ❌       |

この完全なアーキテクチャにより、`npm install -g @openai/codex` から `codex` コマンド実行まで、全プロセスが自動化・最適化されています。