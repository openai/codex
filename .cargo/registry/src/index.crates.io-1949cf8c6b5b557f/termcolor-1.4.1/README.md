termcolor
=========
A simple cross platform library for writing colored text to a terminal. This
library writes colored text either using standard ANSI escape sequences or by
interacting with the Windows console. Several convenient abstractions are
provided for use in single-threaded or multi-threaded command line
applications.

[![Build status](https://github.com/BurntSushi/termcolor/workflows/ci/badge.svg)](https://github.com/BurntSushi/termcolor/actions)
[![crates.io](https://img.shields.io/crates/v/termcolor.svg)](https://crates.io/crates/termcolor)

Dual-licensed under MIT or the [UNLICENSE](https://unlicense.org/).

### Documentation

[https://docs.rs/termcolor](https://docs.rs/termcolor)

### Usage

Run `cargo add termcolor` to add this dependency to your `Cargo.toml` file.

### Organization

The `WriteColor` trait extends the `io::Write` trait with methods for setting
colors or resetting them.

`StandardStream` and `StandardStreamLock` both satisfy `WriteColor` and are
analogous to `std::io::Stdout` and `std::io::StdoutLock`, or `std::io::Stderr`
and `std::io::StderrLock`.

`Buffer` is an in memory buffer that supports colored text. In a parallel
program, each thread might write to its own buffer. A buffer can be printed to
stdout or stderr using a `BufferWriter`. The advantage of this design is that
each thread can work in parallel on a buffer without having to synchronize
access to global resources such as the Windows console. Moreover, this design
also prevents interleaving of buffer output.

`Ansi` and `NoColor` both satisfy `WriteColor` for arbitrary implementors of
`io::Write`. These types are useful when you know exactly what you need. An
analogous type for the Windows console is not provided since it cannot exist.

### Example: using `StandardStream`

The `StandardStream` type in this crate works similarly to `std::io::Stdout`,
except it is augmented with methods for coloring by the `WriteColor` trait. For
example, to write some green text:

```rust
use std::io::{self, Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

fn write_green() -> io::Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
    writeln!(&mut stdout, "green text!")
}
```

### Example: using `BufferWriter`

A `BufferWriter` can create buffers and write buffers to stdout or stderr. It
does *not* implement `io::Write` or `WriteColor` itself. Instead, `Buffer`
implements `io::Write` and `termcolor::WriteColor`.

This example shows how to print some green text to stderr.

```rust
use std::io::{self, Write};
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

fn write_green() -> io::Result<()> {
    let mut bufwtr = BufferWriter::stderr(ColorChoice::Always);
    let mut buffer = bufwtr.buffer();
    buffer.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
    writeln!(&mut buffer, "green text!")?;
    bufwtr.print(&buffer)
}
```

### Automatic color selection

When building a writer with termcolor, the caller must provide a
[`ColorChoice`](https://docs.rs/termcolor/1.*/termcolor/enum.ColorChoice.html)
selection. When the color choice is `Auto`, termcolor will attempt to determine
whether colors should be enabled by inspecting the environment. Currently,
termcolor will inspect the `TERM` and `NO_COLOR` environment variables:

* If `NO_COLOR` is set to any value, then colors will be suppressed.
* If `TERM` is set to `dumb`, then colors will be suppressed.
* In non-Windows environments, if `TERM` is not set, then colors will be
  suppressed.

This decision procedure may change over time.

Currently, `termcolor` does not attempt to detect whether a tty is present or
not. To achieve that, please use
[`std::io::IsTerminal`](https://doc.rust-lang.org/std/io/trait.IsTerminal.html).

### Minimum Rust version policy

This crate's minimum supported `rustc` version is `1.34.0`.

The current policy is that the minimum Rust version required to use this crate
can be increased in minor version updates. For example, if `crate 1.0` requires
Rust 1.20.0, then `crate 1.0.z` for all values of `z` will also require Rust
1.20.0 or newer. However, `crate 1.y` for `y > 0` may require a newer minimum
version of Rust.

In general, this crate will be conservative with respect to the minimum
supported version of Rust.
