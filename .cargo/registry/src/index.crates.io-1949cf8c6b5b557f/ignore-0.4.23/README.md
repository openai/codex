ignore
======
The ignore crate provides a fast recursive directory iterator that respects
various filters such as globs, file types and `.gitignore` files. This crate
also provides lower level direct access to gitignore and file type matchers.

[![Build status](https://github.com/BurntSushi/ripgrep/workflows/ci/badge.svg)](https://github.com/BurntSushi/ripgrep/actions)
[![](https://img.shields.io/crates/v/ignore.svg)](https://crates.io/crates/ignore)

Dual-licensed under MIT or the [UNLICENSE](https://unlicense.org/).

### Documentation

[https://docs.rs/ignore](https://docs.rs/ignore)

### Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
ignore = "0.4"
```

### Example

This example shows the most basic usage of this crate. This code will
recursively traverse the current directory while automatically filtering out
files and directories according to ignore globs found in files like
`.ignore` and `.gitignore`:


```rust,no_run
use ignore::Walk;

for result in Walk::new("./") {
    // Each item yielded by the iterator is either a directory entry or an
    // error, so either print the path or the error.
    match result {
        Ok(entry) => println!("{}", entry.path().display()),
        Err(err) => println!("ERROR: {}", err),
    }
}
```

### Example: advanced

By default, the recursive directory iterator will ignore hidden files and
directories. This can be disabled by building the iterator with `WalkBuilder`:

```rust,no_run
use ignore::WalkBuilder;

for result in WalkBuilder::new("./").hidden(false).build() {
    println!("{:?}", result);
}
```

See the documentation for `WalkBuilder` for many other options.
