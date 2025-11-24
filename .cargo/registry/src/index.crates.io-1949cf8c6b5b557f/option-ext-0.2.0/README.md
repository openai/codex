[![crates.io](https://img.shields.io/crates/v/option-ext.svg)](https://crates.io/crates/option-ext)
[![API documentation](https://docs.rs/option-ext/badge.svg)](https://docs.rs/option-ext/)
![actively developed](https://img.shields.io/badge/maintenance-actively--developed-brightgreen.svg)
![License: MPL-2.0](https://img.shields.io/badge/license-MPL--2.0-orange.svg)

# `option-ext`

## Introduction

This crate extends `Option` with additional methods, currently:

- `contains`
- `map_or2` (as a replacement for `map_or`)
- `map_or_else2` (as a replacement for `map_or_else`)

Its sister crate is [`result-ext`](https://github.com/soc/result-ext), which extends `Result`.

## Requirements

Rust 1.0 or newer.

## Usage

#### Dependency

Add the library as a dependency to your project by inserting

```toml
option-ext = "0.2.0"
```

into the `[dependencies]` section of your Cargo.toml file.

#### Example

```rust
use option_ext::OptionExt;

fn example_contains() {
    let x: Option<u32> = Some(2);
    assert_eq!(x.contains(&2), true);

    let x: Option<u32> = Some(3);
    assert_eq!(x.contains(&2), false);

    let x: Option<u32> = None;
    assert_eq!(x.contains(&2), false);
}

fn example_map_or2() {
    let x = Some("bar");
    assert_eq!(x.map_or2(|v| v.len(), 42), 3);

    let x: Option<&str> = None;
    assert_eq!(x.map_or2(|v| v.len(), 42), 42);
}

fn example_map_or_else2() {
    let k = 23;
    
    let x = Some("bar");
    assert_eq!(x.map_or_else2(|v| v.len(), || 2 * k), 3);
    
    let x: Option<&str> = None;
    assert_eq!(x.map_or_else2(|v| v.len(), || 2 * k), 46);
}
```
