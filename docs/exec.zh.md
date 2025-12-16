# 非交互模式与自动化（codex exec）

> 本文是 `docs/exec.md` 的中文概览版本，简要介绍 `codex exec` 的用法。

`codex exec` 用于在**非交互模式**下运行 Codex，适合：

- CI / 定时任务。
- 快速执行一次性自动化操作。
- 作为其他脚本/工具的一部分。

## 基本用法

```bash
codex exec "解释 src/utils.ts 的逻辑"
```

或从 stdin 接收 prompt：

```bash
echo "请检查当前仓库中的安全隐患" | codex exec
```

特点：

- 不进入全屏 TUI，而是直接在终端打印输出。
- Codex 会在任务完成后退出，适合自动化场景。

## 与 sandbox / 审批的配合

在自动化场景下，建议显式指定 sandbox 和审批策略，例如：

```bash
codex exec --sandbox workspace-write --approval-mode on-request "任务描述"
```

或在配置文件中设置默认策略，再按需通过 CLI 覆盖。

## 典型场景示例

- 代码审查：
  - 在 CI 中对 PR 触发 `codex exec`，生成审查报告，附加到评论。
- 文档与测试：
  - 自动生成或更新部分文档，并在 sandbox 中运行测试。
- 批量重构：
  - 让 Codex 在限定目录下执行一系列重构或迁移操作，并提交 patch。

完整选项与示例请参考英文文档 `docs/exec.md`。

