same-file
=========
A safe and cross platform crate to determine whether two files or directories
are the same.

[![Build status](https://github.com/BurntSushi/same-file/workflows/ci/badge.svg)](https://github.com/BurntSushi/same-file/actions)
[![](http://meritbadge.herokuapp.com/same-file)](https://crates.io/crates/same-file)

Dual-licensed under MIT or the [UNLICENSE](http://unlicense.org).

### Documentation

https://docs.rs/same-file

### Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
same-file = "1"
```

### Example

The simplest use of this crate is to use the `is_same_file` function, which
takes two file paths and returns true if and only if they refer to the same
file:

```rust,no_run
use same_file::is_same_file;

fn main() {
    assert!(is_same_file("/bin/sh", "/usr/bin/sh").unwrap());
}
```

### Minimum Rust version policy

This crate's minimum supported `rustc` version is `1.34.0`.

The current policy is that the minimum Rust version required to use this crate
can be increased in minor version updates. For example, if `crate 1.0` requires
Rust 1.20.0, then `crate 1.0.z` for all values of `z` will also require Rust
1.20.0 or newer. However, `crate 1.y` for `y > 0` may require a newer minimum
version of Rust.

In general, this crate will be conservative with respect to the minimum
supported version of Rust.
