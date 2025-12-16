# 配置参考（Config）

> 本文是 `docs/config.md` 的中文概览版本，主要说明配置思路和常见选项。详细字段和最新更新请以英文原文为准。

Codex 的配置文件默认位于：

- `~/.codex/config.toml`

你也可以在运行时通过 CLI 参数覆盖其中部分配置。

## 配置文件结构概览

典型配置包含以下几类内容：

- 全局行为：日志级别、默认模型、sandbox 策略、审批策略等。
- 身份验证：使用 ChatGPT、API Key 或其他 provider 所需的设置。
- MCP 配置：要连接的 MCP servers、路径、命令等。
- Prompts / 自定义指令：默认系统提示词、slash commands 相关设置。

示意结构：

```toml
[core]
default_model = "gpt-4.1"
sandbox_mode = "workspace-write"

[auth]
provider = "openai"

[mcp_servers.my_server]
command = "..."
args = ["..."]
```

> 实际字段以英文 `config.md` 为准，上例仅示意。

## 覆盖配置：CLI 参数 vs 文件

- 多数选项可以在 CLI 中通过参数覆盖配置文件中的默认值，例如：
  - `codex --model gpt-4.1-mini`
  - `codex --sandbox read-only`
- 一般推荐：
  - 将“日常默认行为”放在 `config.toml`。
  - 临时需求使用 CLI 参数覆盖（例如在某次会话中改用不同模型）。

## Sandbox 与审批相关配置

常见字段包括（名称以英文文档为准）：

- `sandbox_mode`：
  - `read-only`：只读 sandbox。
  - `workspace-write`：允许在当前工作区写入，仍阻止网络访问（推荐）。
  - `danger-full-access`：不使用 sandbox，需谨慎。
- `ask_for_approval` / `approval_policy`：
  - 控制在执行文件写入、命令运行等操作前是否需要你的确认。

这些设置可以在配置文件和 CLI 中配合使用。

## MCP 相关配置

配置结构通常类似：

```toml
[mcp_servers.example]
command = "npx"
args = ["@modelcontextprotocol/inspector", "server"]
env = { KEY = "VALUE" }
```

- 每个 MCP server 使用一个命名段落。
- 可以指定：
  - 启动命令及参数。
  - 环境变量。
  - 其他与连接和重试相关的选项（详见英文文档）。

配置完成后，Codex 在启动时会根据这些配置自动连接 MCP servers。

## 自定义 prompts 与 slash commands

- 可以在配置中定义自定义 prompts，配合 Slash Commands 使用。
- 典型用法：
  - 把常用的“工作流提示词”抽取出来，作为命令复用。
  - 在团队中共享一套经过验证的工作流。

具体格式和示例请参考：

- `docs/prompts.md`
- 以及 `docs/config.md` 中关于 prompts 的部分。

