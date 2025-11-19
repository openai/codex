# codex-linux-sandbox（中文说明）

这个 crate 负责生成：

- 一个 Linux 平台的独立可执行文件 `codex-linux-sandbox`，并随 Node.js 版本的 Codex CLI 一起分发；
- 一个 lib crate，向外暴露可执行文件的业务逻辑为 `run_main()`，以便：
  - `codex-exec` CLI 可以检测其 `arg0` 是否为 `codex-linux-sandbox`，若是，则按 `codex-linux-sandbox` 的方式运行；
  - 同样地，`codex` 多功能 CLI 也可以复用这一逻辑。

