# codex-process-hardening（中文说明）

这个 crate 提供了 `pre_main_hardening()` 函数，设计为在 `main()` 之前调用（通过 `#[ctor::ctor]`），用于执行一系列进程加固步骤，例如：

- 禁用 core dump；
- 在 Linux 和 macOS 上禁用 ptrace attach；
- 移除诸如 `LD_PRELOAD`、`DYLD_*` 之类危险的环境变量。

