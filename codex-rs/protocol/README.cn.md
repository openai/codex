# codex-protocol（中文说明）

这个 crate 定义了 Codex CLI 使用的协议“类型”。它同时涵盖：

- `codex-core` 与 `codex-tui` 之间通信所用的“内部类型”；
- `codex app-server` 对外暴露的“外部类型”。

本 crate 应尽量保持依赖精简。

理想情况下，我们应避免在这里引入“实质性的业务逻辑”；如果需要为类型增加行为，可以在其他 crate 中通过 `Ext` 风格的 trait 扩展。

