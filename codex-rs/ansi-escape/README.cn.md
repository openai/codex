# oai-codex-ansi-escape（中文说明）

一些小的辅助函数，用来封装 <https://crates.io/crates/ansi-to-tui> 提供的功能：

```rust
pub fn ansi_escape_line(s: &str) -> Line<'static>
pub fn ansi_escape<'a>(s: &'a str) -> Text<'a>
```

优点：

- 避免在整个 TUI crate 的作用域里都引入 `ansi_to_tui::IntoText`
- 如果 `IntoText` 返回 `Err`，我们会直接 `panic!()` 并记录日志，这样调用方就不需要自己处理错误

