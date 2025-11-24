# backtrace-rs

[Documentation](https://docs.rs/backtrace)

A library for acquiring backtraces at runtime for Rust. This library aims to
enhance the support of the standard library by providing a programmatic
interface to work with, but it also supports simply easily printing the current
backtrace like libstd's panics.

## Install

```toml
[dependencies]
backtrace = "0.3"
```

## Usage

To simply capture a backtrace and defer dealing with it until a later time,
you can use the top-level `Backtrace` type.

```rust
use backtrace::Backtrace;

fn main() {
    let bt = Backtrace::new();

    // do_some_work();

    println!("{bt:?}");
}
```

If, however, you'd like more raw access to the actual tracing functionality, you
can use the `trace` and `resolve` functions directly.

```rust
fn main() {
    backtrace::trace(|frame| {
        let ip = frame.ip();
        let symbol_address = frame.symbol_address();

        // Resolve this instruction pointer to a symbol name
        backtrace::resolve_frame(frame, |symbol| {
            if let Some(name) = symbol.name() {
                // ...
            }
            if let Some(filename) = symbol.filename() {
                // ...
            }
        });

        true // keep going to the next frame
    });
}
```

# Supported Rust Versions

The `backtrace` crate is a core component of the standard library, and must
at times keep up with the evolution of various platforms in order to serve
the standard library's needs. This often means using recent libraries
that provide unwinding and symbolication for various platforms.
Thus `backtrace` is likely to use recent Rust features or depend on a library
which itself uses them. Its minimum supported Rust version, by policy, is
within a few versions of current stable, approximately "stable - 2".

This policy takes precedence over versions written anywhere else in this repo.

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in backtrace-rs by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
