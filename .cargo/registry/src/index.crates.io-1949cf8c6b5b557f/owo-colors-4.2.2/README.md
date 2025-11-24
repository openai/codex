# <img src="https://jam1.re/img/rust_owo.svg" height="100"> Colors

[![Current Crates.io Version](https://img.shields.io/crates/v/owo-colors.svg)](https://crates.io/crates/owo-colors)
[![docs-rs](https://docs.rs/owo-colors/badge.svg)](https://docs.rs/owo-colors)
![MSRV 1.70+](https://img.shields.io/badge/rustc-1.70+-blue.svg)
![Downloads](https://img.shields.io/crates/d/owo-colors)

A zero-allocation no_std-compatible zero-cost way to add color to your Rust
terminal to make people go owo.

**Supports:**

- [x] All std/core formatters
  - [x] [Display](https://doc.rust-lang.org/std/fmt/trait.Display.html)
  - [x] [Debug](https://doc.rust-lang.org/std/fmt/trait.Debug.html)
  - [x] [Octal](https://doc.rust-lang.org/std/fmt/trait.Octal.html)
  - [x] [LowerHex](https://doc.rust-lang.org/std/fmt/trait.LowerHex.html)
  - [x] [UpperHex](https://doc.rust-lang.org/std/fmt/trait.UpperHex.html)
  - [x] [Pointer](https://doc.rust-lang.org/std/fmt/trait.Pointer.html)
  - [x] [Binary](https://doc.rust-lang.org/std/fmt/trait.Binary.html)
  - [x] [LowerExp](https://doc.rust-lang.org/std/fmt/trait.LowerExp.html)
  - [x] [UpperExp](https://doc.rust-lang.org/std/fmt/trait.UpperExp.html)
- [x] Optional checking for if a terminal supports colors
  - [x] Enabled for CI
  - [x] Disabled by default for non-terminal outputs
  - [x] Overridable by `NO_COLOR`/`FORCE_COLOR` environment variables
  - [x] Overridable programmatically via [`set_override`](https://docs.rs/owo-colors/latest/owo_colors/fn.set_override.html)
- [x] Dependency-less by default
- [x] 100% safe code
- [x] Most functions are `const`
- [x] Hand picked names for all ANSI (4-bit) and Xterm (8-bit) colors
- [x] Support for RGB colors
- [x] Set colors at compile time by generics or at runtime by value
- [x] All ANSI colors
  - [x] Basic support (normal and bright variants)
  - [x] Xterm support (high compatibility and 256 colors)
  - [x] Truecolor support (modern, 48-bit color)
- [x] Styling (underline, strikethrough, etc)

owo-colors is also more-or-less a drop-in replacement for
[colored](https://crates.io/crates/colored), allowing colored to work in a
no_std environment. No allocations or dependencies required because embedded
systems deserve to be pretty too uwu.

To add to your Cargo.toml:

```toml
owo-colors = "4"
```

## Example

```rust
use owo_colors::OwoColorize;

fn main() {
    // Foreground colors
    println!("My number is {:#x}!", 10.green());
    // Background colors
    println!("My number is not {}!", 4.on_red());
}
```

## Generic colors

```rust
use owo_colors::OwoColorize;
use owo_colors::colors::*;

fn main() {
    // Generically color
    println!("My number might be {}!", 4.fg::<Black>().bg::<Yellow>());
}
```

## Stylize

```rust
use owo_colors::OwoColorize;

println!("{}", "strikethrough".strikethrough());
```

## Only Style on Supported Terminals

```rust
use owo_colors::{OwoColorize, Stream::Stdout};

println!(
    "{}",
    "colored blue if a supported terminal"
        .if_supports_color(Stdout, |text| text.bright_blue())
);
```

Supports `NO_COLOR`/`FORCE_COLOR` environment variables, checks if it's a tty,
checks if it's running in CI (and thus likely supports color), and checks which
terminal is being used. (Note: requires `supports-colors` feature)

## Minimum supported Rust version (MSRV)

The MSRV of owo-colors is **Rust 1.70**.

This library has a conservative MSRV policy — Rust versions from the last 12 months will be supported (features available in newer versions will be gated by `build.rs`), the MSRV will only be bumped if required, and if the MSRV is bumped it will always occur with a new minor version.
