# ansi-to-tui

![drone build](https://img.shields.io/drone/build/uttarayan21/ansi-to-tui?server=https%3A%2F%2Fdrone.uttarayan.me)
[![github build](https://github.com/uttarayan21/ansi-to-tui/actions/workflows/build.yaml/badge.svg)][ansi-to-tui]
[![downloads](https://img.shields.io/crates/d/ansi-to-tui)](https://crates.io/crates/ansi-to-tui)

A nom parser to parse text with ANSI color codes and turn them into [`ratatui::text::Text`][Text].

For people still using [tui-rs](docs.rs/tui) use version `v2.*` for people migrating to [ratatui](docs.rs/ratatui) use version `v3.*`
I recommend switching over to ratatui since tui-rs is currently unmaintained.

| Color  | Supported | Examples                 |
| ------ | :-------: | ------------------------ |
| 24 bit |     ✓     | `\x1b[38;2;<R>;<G>;<B>m` |
| 8 bit  |     ✓     | `\x1b[38;5;<N>m`         |
| 4 bit  |     ✓     | `\x1b[30..37;40..47m`    |

## Example

```rust
use ansi_to_tui::IntoText;
let buffer = std::fs::read("ascii/text.ascii").unwrap();
let output = buffer.into_text();
```

[Text]: https://docs.rs/ratatui/latest/ratatui/text/struct.Text.html
[ansi-to-tui]: https://github.com/uttarayan21/ansi-to-tui
