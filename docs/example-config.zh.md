# 配置示例（Example Config）

> 本文是 `docs/example-config.md` 的中文概览版本，示例字段以英文原文为准。

`example-config.md` 展示了一份较完整的 `~/.codex/config.toml` 示例，方便你快速了解各配置项的组合方式。

## 示例中常见的内容

通常包含：

- 默认模型与 provider 配置。
- sandbox 策略与审批策略。
- MCP servers 配置示例。
- Slash commands / prompts 相关示例。
- 日志、追踪（如 OpenTelemetry）相关配置。

## 如何使用这些示例

建议做法：

1. 复制示例内容到本地 `~/.codex/config.toml`。
2. 根据自己的环境修改：
   - API Key / provider 类型。
   - 沙箱策略（例如从 `danger-full-access` 改为 `workspace-write`）。
   - MCP server 命令及路径。
3. 在终端运行 `codex`，观察启动时打印的配置摘要是否符合预期。

遇到不理解的字段时，可以回看：

- `docs/config.md`（英文详细解释）。
- 以及相关的 topic 文档（如 `docs/sandbox.md`、`docs/exec.md`）。

