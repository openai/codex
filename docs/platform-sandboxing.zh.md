# 平台级沙箱机制（Platform Sandboxing）

> 本文是 `docs/platform-sandboxing.md` 的中文概览版本，介绍不同平台上的沙箱实现。技术细节以英文原文为准。

Codex 在不同操作系统上使用不同的底层沙箱技术，以控制：

- 文件系统访问范围。
- 网络访问。
- 进程与系统调用级别的能力。

## macOS：Seatbelt

- 使用 macOS 的 Seatbelt 机制对子进程进行限制。
- 通过策略文件/规则限定：
  - 可访问的目录（如当前 workspace）。
  - 禁止网络访问等。

## Linux：Landlock + seccomp

- 使用 Landlock 控制文件系统访问。
- 使用 seccomp（以及相关 crate）限制系统调用。
- 配合 Codex 自身的工作目录/可写目录策略，从两层确保安全性。

## Windows：受限令牌等机制

- 使用受限令牌（restricted token）等 Windows 安全机制。
- 将 Codex 启动的子进程放在更受限的安全上下文中。

## 与 CLI 选项的关系

用户视角下，主要通过以下概念来控制沙箱：

- `--sandbox` / 配置中的 `sandbox_mode`：
  - `read-only`、`workspace-write`、`danger-full-access` 等。
- Codex 会根据当前平台选择对应的底层实现：
  - 例如在 Linux 下组合使用 Landlock + seccomp。

更详细的策略描述与示意图请参见英文文档 `docs/platform-sandboxing.md` 与 `docs/sandbox.md`。

