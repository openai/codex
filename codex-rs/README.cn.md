# Codex CLI（Rust 实现，中文说明）

我们提供 Codex CLI 的原生可执行版本，以确保安装时几乎没有额外依赖。

## 安装 Codex

目前最简单的安装方式是通过 `npm`：

```shell
npm i -g @openai/codex
codex
```

你也可以通过 Homebrew 安装（`brew install --cask codex`），或者直接从我们的 [GitHub Releases](https://github.com/openai/codex/releases) 下载对应平台的二进制发行版。

## 文档快速入口

- 第一次使用 Codex？可以先阅读 [`docs/getting-started.md`](../docs/getting-started.md)，了解基础用法、快捷键和会话管理。
- 已经在日常开发中使用 Codex，希望获得更细粒度的控制？建议查看 [`docs/advanced.md`](../docs/advanced.md) 以及配置参考 [`docs/config.md`](../docs/config.md)。

## Rust CLI 有哪些新特性

Rust 实现是现在的主力 Codex CLI，也是默认体验。它包含许多旧的 TypeScript CLI 从未支持的能力。

### 配置（Config）

Codex 提供了一套较为丰富的配置选项。需要注意的是，Rust 版本 CLI 使用的是 `config.toml`（而不是旧版的 `config.json`）。详细说明见 [`docs/config.md`](../docs/config.md)。

### Model Context Protocol 支持

#### 作为 MCP 客户端

Codex CLI 可以作为 MCP 客户端，在启动时连接到多个 MCP server。具体配置方式见 [`配置文档`](../docs/config.md#mcp_servers)。

#### 作为 MCP server（实验性）

通过 `codex mcp-server` 可以把 Codex 本身以 MCP _server_ 的形式暴露出去，从而让 _其他_ MCP 客户端把 Codex 当作一个工具使用。

你可以使用 [`@modelcontextprotocol/inspector`](https://github.com/modelcontextprotocol/inspector) 来快速试用：

```shell
npx @modelcontextprotocol/inspector codex mcp-server
```

同时可以使用 `codex mcp` 来在 `config.toml` 中添加/列出/查看/删除 MCP server 启动配置，用 `codex mcp-server` 直接运行 MCP server。

### 通知（Notifications）

你可以通过配置一个脚本，在 agent 完成一个回合时自动执行，从而实现桌面通知等效果。详见 [通知配置文档](../docs/config.md#notify)，其中包含了在 macOS 上通过 [terminal-notifier](https://github.com/julienXX/terminal-notifier) 实现桌面通知的完整示例。

### `codex exec`：以脚本方式/非交互方式运行 Codex

如果需要以非交互方式运行 Codex，可以使用：

```shell
codex exec PROMPT
```

也可以通过 `stdin` 传入 prompt。Codex 会持续执行任务，直到认为已经完成并退出。所有输出会直接打印到终端；如需查看更多内部细节，可以设置 `RUST_LOG` 环境变量。

### 实验 Codex Sandbox

为了测试在 Codex 提供的沙箱中执行命令的效果，CLI 提供了以下子命令：

```shell
# macOS
codex sandbox macos [--full-auto] [--log-denials] [COMMAND]...

# Linux
codex sandbox linux [--full-auto] [COMMAND]...

# Windows
codex sandbox windows [--full-auto] [COMMAND]...

# 旧别名
codex debug seatbelt [--full-auto] [--log-denials] [COMMAND]...
codex debug landlock [--full-auto] [COMMAND]...
```

### 通过 `--sandbox` 选择沙箱策略

Rust CLI 暴露了一个专门的 `--sandbox`（或 `-s`）参数，让你无需通过通用的 `-c/--config` 也能快速切换沙箱策略：

```shell
# 使用默认的只读沙箱运行 Codex
codex --sandbox read-only

# 允许 agent 在当前工作区内写文件，但仍然禁止网络访问
codex --sandbox workspace-write

# 危险！完全关闭沙箱（只在你的环境本身已经被隔离时使用，例如容器里）
codex --sandbox danger-full-access
```

同样的配置也可以写入 `~/.codex/config.toml` 的顶层键 `sandbox_mode = "MODE"`，例如 `sandbox_mode = "workspace-write"`。

## 代码结构概览

`codex-rs` 是一个 Cargo workspace 的根目录，里面包含了不少实验性代码，但有几个核心 crate：

- [`core/`](./core)：包含 Codex 的业务逻辑。理想情况下，它可以发展为一个通用的 library crate，方便其他 Rust/本地应用集成 Codex。
- [`exec/`](./exec)：“无头” CLI，用于自动化场景。
- [`tui/`](./tui)：使用 [Ratatui](https://ratatui.rs/) 构建的全屏终端 UI。
- [`cli/`](./cli)：多功能 CLI，提供上述 CLI 功能作为子命令。

