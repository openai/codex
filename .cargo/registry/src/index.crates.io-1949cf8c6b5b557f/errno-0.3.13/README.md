# errno [![CI](https://github.com/lambda-fairy/rust-errno/actions/workflows/main.yml/badge.svg)](https://github.com/lambda-fairy/rust-errno/actions/workflows/main.yml) [![Cargo](https://img.shields.io/crates/v/errno.svg)](https://crates.io/crates/errno)

Cross-platform interface to the [`errno`][errno] variable. Works on Rust 1.56 or newer.

Documentation is available at <https://docs.rs/errno>.

[errno]: https://en.wikipedia.org/wiki/Errno.h


## Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
errno = "*"
```


## Comparison with `std::io::Error`

The standard library provides [`Error::last_os_error`][last_os_error] which fetches `errno` in the same way.

This crate provides these extra features:

- No heap allocations
- Optional `#![no_std]` support
- A `set_errno` function

[last_os_error]: https://doc.rust-lang.org/std/io/struct.Error.html#method.last_os_error


## Examples

```rust
extern crate errno;
use errno::{Errno, errno, set_errno};

// Get the current value of errno
let e = errno();

// Set the current value of errno
set_errno(e);

// Extract the error code as an i32
let code = e.0;

// Display a human-friendly error message
println!("Error {}: {}", code, e);
```


## `#![no_std]`

Enable `#![no_std]` support by disabling the default `std` feature:

```toml
[dependencies]
errno = { version = "*", default-features = false }
```

The `Error` impl will be unavailable.
