# Codex CLI (Rust Implementation)

We provide Codex CLI as a standalone, native executable to ensure a zero-dependency install.

## 🏗️ Architecture | アーキテクチャ

### System Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           User Interface Layer                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐ │
│  │   TUI    │  │   CLI    │  │ VS Code  │  │  Cursor  │  │ Windsurf │ │
│  │ (Ratatui)│  │  (Exec)  │  │Extension │  │Extension │  │Extension │ │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘ │
│       │             │             │             │             │        │
│       └─────────────┴─────────────┴─────────────┴─────────────┘        │
│                                   │                                     │
└───────────────────────────────────┼─────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         Codex Core (Rust)                                │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                     Auto-Orchestration Layer                      │  │
│  │  ┌──────────────┐  ┌───────────────┐  ┌───────────────────────┐ │  │
│  │  │TaskAnalyzer  │→ │AutoOrchestrator│→ │CollaborationStore     │ │  │
│  │  │(5-factor)    │  │(Plan & Execute)│  │(DashMap/Thread-safe)  │ │  │
│  │  └──────────────┘  └───────────────┘  └───────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                   │                                     │
│  ┌───────────────────────────────┼─────────────────────────────────┐  │
│  │          Agent Runtime         │                                  │  │
│  │  ┌──────────────┬──────────────┴──────────────┬──────────────┐  │  │
│  │  │CodeExpert    │SecurityExpert│TestingExpert │DocsExpert    │  │  │
│  │  ├──────────────┼──────────────┼──────────────┼──────────────┤  │  │
│  │  │DeepResearcher│DebugExpert   │PerfExpert    │General       │  │  │
│  │  └──────────────┴──────────────┴──────────────┴──────────────┘  │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                                   │                                     │
│  ┌───────────────────────────────┼─────────────────────────────────┐  │
│  │          MCP Integration       │                                  │  │
│  │  ┌────────────────────────────┴──────────────────────────────┐  │  │
│  │  │ MCP Client                  │  MCP Server                  │  │  │
│  │  │ (Connect to external MCPs)  │  (Expose Codex as MCP tool)  │  │  │
│  │  └─────────────────────────────┴──────────────────────────────┘  │  │
│  └─────────────────────────────────────────────────────────────────┘  │
│                                   │                                     │
│  ┌───────────────────────────────┼─────────────────────────────────┐  │
│  │        Execution Layer         │                                  │  │
│  │  ┌────────────────┬────────────┴─────────┬────────────────────┐  │  │
│  │  │Shell Executor  │File Operations       │Git Tooling         │  │  │
│  │  ├────────────────┼──────────────────────┼────────────────────┤  │  │
│  │  │Sandbox (macOS/Linux) │Apply Patch    │Process Hardening   │  │  │
│  │  └────────────────┴──────────────────────┴────────────────────┘  │  │
│  └─────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────┬─────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                       External Services Layer                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ OpenAI API   │  │  Anthropic   │  │  Web Search  │  │ MCP Tools  │ │
│  │ (GPT-4/5)    │  │  (Claude)    │  │ (DuckDuckGo) │  │ (Custom)   │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

### Web GUI | コントロールセンター

- `codex-gui`（Rust/Axum）を追加し、Codex CLI の主要サブコマンドを HTTP API として公開。
- `gui/frontend`（Node.js/Vite + React）で操作パネルを提供し、Playbook（Ask/Delegate/Research/Review/Audit）をワンクリック実行。
- 実行ログ（stdout/stderr/exit code/実行時間）をリアルタイムで可視化し、履歴パネルにスタック。
- `.env`不要で、`CODEX_GUI_CLI_PATH` と `VITE_API_URL` により CLI パス/ポートを柔軟に切り替え可能。
- セキュアな CORS 設定と `POST /api/actions/:id/execute` での必須フィールド検証により、GUI からの誤操作を防止。

### Auto-Orchestration Flow | 自律オーケストレーションフロー

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          User Input                                      │
│                  "Implement secure authentication                        │
│                   with tests and documentation"                          │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    TaskAnalyzer (5-Factor Analysis)                      │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │ 1. Keyword Density (0.30) ─→ "auth", "test", "docs"              │  │
│  │ 2. Domain Count (0.25)    ─→ Security + Testing + Docs domains   │  │
│  │ 3. Verb Count (0.20)      ─→ "implement", "create", "write"      │  │
│  │ 4. Length Factor (0.15)   ─→ Multi-sentence request              │  │
│  │ 5. Question Marks (0.10)  ─→ Complexity indicators               │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                    Complexity Score: 0.82 (Threshold: 0.7)               │
│                         ✅ TRIGGERS ORCHESTRATION                        │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│              AutoOrchestrator (Plan Generation)                          │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │ Execution Plan:                                                   │  │
│  │  Task 1: Security Review        → SecurityExpert   (Parallel)    │  │
│  │  Task 2: Implement Auth Logic   → CodeExpert       (Parallel)    │  │
│  │  Task 3: Generate Tests         → TestingExpert    (Parallel)    │  │
│  │  Task 4: Write Documentation    → DocsExpert       (Sequential)  │  │
│  └───────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                  Parallel Execution (Tasks 1-3)                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                  │
│  │SecurityExpert│  │  CodeExpert  │  │TestingExpert │                  │
│  │              │  │              │  │              │                  │
│  │ • CVE Scan   │  │ • JWT impl   │  │ • Unit tests │                  │
│  │ • Dep audit  │  │ • Password   │  │ • E2E tests  │                  │
│  │ • Code scan  │  │   hashing    │  │ • Coverage   │                  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘                  │
│         │                 │                 │                          │
│         └─────────────────┴─────────────────┘                          │
│                           │                                             │
│                           ▼                                             │
│              ┌─────────────────────────────┐                            │
│              │  CollaborationStore (Sync)  │                            │
│              │  • Share context            │                            │
│              │  • Aggregate results        │                            │
│              └─────────────┬───────────────┘                            │
│                            │                                            │
│                            ▼                                            │
│              ┌─────────────────────────────┐                            │
│              │     DocsExpert              │                            │
│              │  (Sequential - uses results)│                            │
│              │  • API docs                 │                            │
│              │  • Security guide           │                            │
│              │  • Test coverage report     │                            │
│              └─────────────────────────────┘                            │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      Aggregated Result                                   │
│  ✅ Authentication implemented with JWT                                  │
│  ✅ Security scan passed (0 critical issues)                             │
│  ✅ Test coverage: 94% (42 tests passing)                                │
│  ✅ Documentation generated (API + Security guide)                       │
│  ⚡ Total time: 3.2 minutes (2.6x faster than sequential)                │
└─────────────────────────────────────────────────────────────────────────┘
```

### Technology Stack | 技術スタック

| Layer | Technology | Purpose |
|-------|-----------|---------|
| **Core Runtime** | Rust (Tokio) | 非同期実行・パフォーマンス最適化 / Async execution & performance |
| **UI (Terminal)** | Ratatui | インタラクティブTUI / Interactive terminal UI |
| **UI (IDE)** | TypeScript | VS Code/Cursor/Windsurf拡張 / IDE extensions |
| **SDK** | Node.js (N-API) | Rust↔Node.js連携 / Rust-Node.js bridge |
| **Protocol** | MCP (JSON-RPC) | エージェント間通信 / Inter-agent communication |
| **Orchestration** | DashMap | スレッドセーフな状態管理 / Thread-safe state management |
| **Sandbox** | Seatbelt/Landlock | macOS/Linuxセキュリティ / Security isolation |
| **AI Models** | OpenAI/Anthropic | GPT-4/5, Claude / LLM inference |
| **Web Search** | DuckDuckGo API | Deep Research機能 / Research capabilities |

### Key Features | 主要機能

| Feature | Description (EN) | 説明 (JA) |
|---------|------------------|-----------|
| **Auto-Orchestration** | Automatic task complexity analysis (5-factor scoring) and parallel sub-agent coordination | タスク複雑度の自動分析（5要素スコアリング）と並列サブエージェント協調 |
| **Performance** | 2.6x speedup via parallel execution | 並列実行による2.6倍の高速化 |
| **MCP Integration** | Bi-directional MCP support (client & server) | 双方向MCPサポート（クライアント＆サーバー） |
| **Deep Research** | Multi-source research with citations and contradiction detection | 複数ソースからの引用付き調査と矛盾検出 |
| **Security** | Sandbox isolation (Seatbelt/Landlock) with approval policies | サンドボックス分離（Seatbelt/Landlock）と承認ポリシー |
| **Cross-platform** | Windows, macOS, Linux support | Windows、macOS、Linux対応 |
| **Multi-IDE** | VS Code, Cursor, Windsurf extensions | VS Code、Cursor、Windsurf拡張 |

---

## Installing Codex

Today, the easiest way to install Codex is via `npm`:

```shell
npm i -g @openai/codex
codex
```

You can also install via Homebrew (`brew install --cask codex`) or download a platform-specific release directly from our [GitHub Releases](https://github.com/openai/codex/releases).

## Documentation quickstart

- First run with Codex? Follow the walkthrough in [`docs/getting-started.md`](../docs/getting-started.md) for prompts, keyboard shortcuts, and session management.
- Already shipping with Codex and want deeper control? Jump to [`docs/advanced.md`](../docs/advanced.md) and the configuration reference at [`docs/config.md`](../docs/config.md).

## What's new in the Rust CLI

The Rust implementation is now the maintained Codex CLI and serves as the default experience. It includes a number of features that the legacy TypeScript CLI never supported.

### Config

Codex supports a rich set of configuration options. Note that the Rust CLI uses `config.toml` instead of `config.json`. See [`docs/config.md`](../docs/config.md) for details.

### Model Context Protocol Support

#### MCP client

Codex CLI functions as an MCP client that allows the Codex CLI and IDE extension to connect to MCP servers on startup. See the [`configuration documentation`](../docs/config.md#mcp_servers) for details.

#### MCP server (experimental)

Codex can be launched as an MCP _server_ by running `codex mcp-server`. This allows _other_ MCP clients to use Codex as a tool for another agent.

Use the [`@modelcontextprotocol/inspector`](https://github.com/modelcontextprotocol/inspector) to try it out:

```shell
npx @modelcontextprotocol/inspector codex mcp-server
```

Use `codex mcp` to add/list/get/remove MCP server launchers defined in `config.toml`, and `codex mcp-server` to run the MCP server directly.

### Notifications

You can enable notifications by configuring a script that is run whenever the agent finishes a turn. The [notify documentation](../docs/config.md#notify) includes a detailed example that explains how to get desktop notifications via [terminal-notifier](https://github.com/julienXX/terminal-notifier) on macOS.

### `codex exec` to run Codex programmatically/non-interactively

To run Codex non-interactively, run `codex exec PROMPT` (you can also pass the prompt via `stdin`) and Codex will work on your task until it decides that it is done and exits. Output is printed to the terminal directly. You can set the `RUST_LOG` environment variable to see more about what's going on.

### Experimenting with the Codex Sandbox

To test to see what happens when a command is run under the sandbox provided by Codex, we provide the following subcommands in Codex CLI:

```
# macOS
codex sandbox macos [--full-auto] [COMMAND]...

# Linux
codex sandbox linux [--full-auto] [COMMAND]...

# Windows
codex sandbox windows [--full-auto] [COMMAND]...

# Legacy aliases
codex debug seatbelt [--full-auto] [COMMAND]...
codex debug landlock [--full-auto] [COMMAND]...
```

### Selecting a sandbox policy via `--sandbox`

The Rust CLI exposes a dedicated `--sandbox` (`-s`) flag that lets you pick the sandbox policy **without** having to reach for the generic `-c/--config` option:

```shell
# Run Codex with the default, read-only sandbox
codex --sandbox read-only

# Allow the agent to write within the current workspace while still blocking network access
codex --sandbox workspace-write

# Danger! Disable sandboxing entirely (only do this if you are already running in a container or other isolated env)
codex --sandbox danger-full-access
```

The same setting can be persisted in `~/.codex/config.toml` via the top-level `sandbox_mode = "MODE"` key, e.g. `sandbox_mode = "workspace-write"`.

## Code Organization

This folder is the root of a Cargo workspace. It contains quite a bit of experimental code, but here are the key crates:

- [`core/`](./core) contains the business logic for Codex. Ultimately, we hope this to be a library crate that is generally useful for building other Rust/native applications that use Codex.
- [`exec/`](./exec) "headless" CLI for use in automation.
- [`tui/`](./tui) CLI that launches a fullscreen TUI built with [Ratatui](https://ratatui.rs/).
- [`cli/`](./cli) CLI multitool that provides the aforementioned CLIs via subcommands.
