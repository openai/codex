<div align="center">
  <h1><code>is-terminal</code></h1>

  <p>
    <strong>Test whether a given stream is a terminal</strong>
  </p>

  <p>
    <a href="https://github.com/sunfishcode/is-terminal/actions?query=workflow%3ACI"><img src="https://github.com/sunfishcode/is-terminal/workflows/CI/badge.svg" alt="Github Actions CI Status" /></a>
    <a href="https://crates.io/crates/is-terminal"><img src="https://img.shields.io/crates/v/is-terminal.svg" alt="crates.io page" /></a>
    <a href="https://docs.rs/is-terminal"><img src="https://docs.rs/is-terminal/badge.svg" alt="docs.rs docs" /></a>
  </p>
</div>

As of Rust 1.70, most users should use the [`IsTerminal`] trait in the Rust
standard library instead of this crate.

On Unix platforms, this crate now uses libc, so that the implementation
matches what's in std. Users wishing to use the rustix-based implementation
can use the [rustix-is-terminal] crate instead.

[rustix-is-terminal]: https://crates.io/crates/rustix-is-terminal

<hr>

is-terminal is a simple utility that answers one question:

> Is this a terminal?

A "terminal", also known as a "tty", is an I/O device which may be interactive
and may support color and other special features. This crate doesn't provide
any of those features; it just answers this one question.

On Unix-family platforms, this is effectively the same as the [`isatty`]
function for testing whether a given stream is a terminal, though it accepts
high-level stream types instead of raw file descriptors.

On Windows, it uses a variety of techniques to determine whether the given
stream is a terminal.

This crate is derived from [the atty crate] with [PR \#51] bug fix and
[PR \#54] port to windows-sys applied. The only additional difference is that
the atty crate only accepts stdin, stdout, or stderr, while this crate accepts
any stream. In particular, this crate does not access any stream that is not
passed to it, in accordance with [I/O safety].

[PR \#51]: https://github.com/softprops/atty/pull/51
[PR \#54]: https://github.com/softprops/atty/pull/54

## Example

```rust
use is_terminal::IsTerminal;

fn main() {
    if std::io::stdout().is_terminal() {
        println!("Stdout is a terminal");
    } else {
        println!("Stdout is not a terminal");
    }
}
```

## Testing

This library is tested on both Unix-family and Windows platforms.

To test it on a platform manually, use the provided `stdio` example program.
When run normally, it prints this:

```bash
$ cargo run --example stdio
stdin? true
stdout? true
stderr? true
```

To test stdin, pipe some text to the program:

```bash
$ cat | cargo run --example stdio
stdin? false
stdout? true
stderr? true
```

To test stdout, pipe the program to something:

```bash
$ cargo run --example stdio | cat
stdin? true
stdout? false
stderr? true
```

To test stderr, pipe the program to something redirecting stderr:

```bash
$ cargo run --example stdio 2>&1 | cat
stdin? true
stdout? false
stderr? false
```

# Minimum Supported Rust Version (MSRV)

This crate currently works on the version of [Rust on Debian stable], which is
currently Rust 1.63. This policy may change in the future, in minor version
releases, so users using a fixed version of Rust should pin to a specific
version of this crate.

[`isatty`]: https://man7.org/linux/man-pages/man3/isatty.3.html
[the atty crate]: https://crates.io/crates/atty
[I/O safety]: https://github.com/rust-lang/rfcs/blob/master/text/3128-io-safety.md
[Rust on Debian stable]: https://packages.debian.org/stable/rust/rustc
[`IsTerminal`]: https://doc.rust-lang.org/stable/std/io/trait.IsTerminal.html
