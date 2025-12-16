# 提示词与 Slash Commands（Prompts & Slash Commands）

> 本文是 `docs/prompts.md` 的中文概览版本，重点说明如何管理和复用提示词。

## 为什么要抽象提示词

在日常使用中，你可能会重复给 Codex 下达类似指令，例如：

- “帮我做一次安全审计并输出报告”。
- “分析这个 PR，指出风险并给出建议”。

把这些常用流程抽象为可复用的“提示模版”，可以：

- 减少重复输入。
- 让团队成员共享一套最佳实践。

## Slash Commands

Codex 支持在对话中使用形如 `/命令名` 的 Slash Command：

- 在输入框键入 `/` 可以触发命令列表。
- 选择某个命令后，会将对应的提示模版插入输入框，或直接执行相应逻辑。

你可以在配置文件 `config.toml` 中：

- 定义新的命令。
- 指定每个命令对应的提示文本、参数占位符等。

## 自定义 prompts 的常见用法

示例场景：

- `/security_review`：针对当前仓库做安全审计。
- `/refactor_service`：重构某个服务并自动更新相关测试。
- `/doc_update`：根据代码变更更新文档。

在 prompts 模版中可以：

- 使用占位符来引用当前文件/选中文本/额外参数。
- 明确约束输出格式（例如要求生成 Markdown 报告或 git patch）。

具体配置格式和示例请参考英文文档 `docs/prompts.md` 与 `docs/config.md` 中相关章节。

