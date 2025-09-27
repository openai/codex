# oai-codex-ansi-escape

Small helper functions that wrap functionality from
<https://crates.io/crates/ansi-to-tui>:

```rust
pub fn ansi_escape_line(s: &str) -> Line<'static>
pub fn ansi_escape<'a>(s: &'a str) -> Text<'a>
```

Advantages:

- `ansi_to_tui::IntoText` is not in scope for the entire TUI crate
- we `panic!()` and log if `IntoText` returns an `Err` and log it so that
  the caller does not have to deal with it
