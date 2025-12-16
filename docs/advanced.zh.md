# 高级用法概览（Advanced）

> 本文是 `docs/advanced.md` 的中文概览版本，并非逐字翻译。细节配置和最新内容请以英文原文为准。

## 本文涵盖什么

- 如何开启更详细的日志与调试输出。
- 如何使用 Model Context Protocol（MCP）等高级集成能力。
- 在 CI/自动化场景中更精细地控制 Codex 的行为。

## 调试与日志

- 可以通过环境变量 `RUST_LOG` 控制日志级别，例如：
  - `RUST_LOG=info codex`
  - `RUST_LOG=trace codex exec "任务描述"`
- 高日志级别在排查 sandbox、网络或工具调用问题时特别有用。

## 与 MCP 集成

- Codex 既可以作为 MCP **客户端**，也可以作为 MCP **服务器**：
  - 作为客户端：在配置文件中声明 MCP servers，Codex 会在启动时连接它们。
  - 作为服务器：通过 `codex mcp-server` 将 Codex 暴露给其他 MCP 客户端使用。
- MCP 能力适合：
  - 把 Codex 接入现有工具生态（数据库、文档系统、自定义 API 等）。
  - 用 Codex 作为一个“可编程工具”嵌入到更大的智能体系统中。

## 高级配置和特性开关

- 部分实验性能力通过“特性开关（feature flags）”控制。
- 可以在配置文件中启用，也可以在 CLI 中传入参数（例如通过 `--feature` 一类的选项）。
- 建议：
  - 在个人环境中先试用实验特性。
  - 在团队或 CI 环境中只开启已经验证稳定的特性。

## 结合 CI / 自动化

- 配合 `codex exec` 可以在 CI 中：
  - 自动执行代码审查或安全检查。
  - 自动生成文档、测试或迁移脚本（在 sandbox 下运行）。
- 建议：
  - 在 CI 中使用严格的 sandbox 策略和审批策略（例如 `workspace-write`）。
  - 对 Codex 输出的更改使用普通的代码评审流程（review + 测试）。

## 进一步阅读

- 英文完整文档：`docs/advanced.md`
- 非交互运行：`docs/exec.md`
- MCP 相关：`docs/config.md` 中关于 MCP 的章节。

