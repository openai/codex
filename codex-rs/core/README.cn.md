# codex-core（中文说明）

这个 crate 实现了 Codex 的业务逻辑，供各种用 Rust 编写的 Codex UI 使用。

## 依赖与运行环境

`codex-core` 假定运行环境中存在一些辅助工具，目前的支持矩阵如下。

### macOS

要求系统中存在 `/usr/bin/sandbox-exec`。

### Linux

要求包含 `codex-core` 的可执行文件在 `arg0` 为 `codex-linux-sandbox` 时，运行等价于 `codex sandbox linux` 的命令（旧别名为 `codex debug landlock`）。具体细节见 `codex-arg0` crate。

### 所有平台

要求包含 `codex-core` 的可执行文件在 `arg1` 为 `--codex-run-as-apply-patch` 时，模拟虚拟的 `apply_patch` CLI。详情同样参见 `codex-arg0` crate。

