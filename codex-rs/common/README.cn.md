# codex-common（中文说明）

这个 crate 用于存放在工作区中多个 crate 间共享的工具代码，但又不适合放进 `core` 的那一类公用功能。

对于比较窄的工具特性，推荐的模式是：

- 在 `Cargo.toml` 的 `[features]` 下新增一个 feature；
- 在 `lib.rs` 中通过 `#[cfg]` 把相关代码挂在对应 feature 上，只在需要时编译。

