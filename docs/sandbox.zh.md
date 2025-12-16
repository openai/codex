# Sandbox 与审批模式（Sandbox & Approvals）

> 本文是 `docs/sandbox.md` 的中文概览版本，结合 `docs/platform-sandboxing.md` 一起阅读效果更好。

Codex 默认在**受限环境**下运行你的代码和命令，以降低对本机和网络的风险。

## 核心概念

- **sandbox 模式**：决定 Codex 作用域内的“能力边界”，例如：
  - 是否允许写文件。
  - 是否允许访问网络。
- **审批模式**：决定在执行某些敏感操作前，是否需要你的明确确认。

## 常见 sandbox 模式

具体名称以实现为准，一般包括：

- `read-only`：只读访问项目文件，不允许写入。
- `workspace-write`：允许在当前工作区写入，阻止访问其他路径（和网络）。
- `danger-full-access`：不使用 sandbox，拥有较高权限，仅在安全受控环境使用。

你可以通过：

- CLI：`codex --sandbox workspace-write`
- 配置：在 `config.toml` 中设置 `sandbox_mode`。

## 审批策略

常见审批策略（名称以实现为准）：

- `on-request`：在写文件或执行命令前弹出确认。
- `full-auto`：允许 Codex 自动执行所有操作（仍受 sandbox 限制）。
- `never`：在某些模式下可能表示“不需要额外审批”，需谨慎使用。

在 TUI 中或通过 CLI，你可以选择不同的审批策略，以平衡效率与安全性。

## 与平台沙箱的关系

- 用户看到的是抽象的 `sandbox_mode`，而底层会根据平台采用不同技术：
  - macOS 上使用 Seatbelt。
  - Linux 上使用 Landlock + seccomp。
  - Windows 上使用受限令牌等机制。
- 细节见 `docs/platform-sandboxing.md`。

