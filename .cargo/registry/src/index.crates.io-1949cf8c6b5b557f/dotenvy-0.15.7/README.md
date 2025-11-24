# dotenvy

[![Crates.io](https://img.shields.io/crates/v/dotenvy.svg)](https://crates.io/crates/dotenvy)
[![msrv
1.56.1](https://img.shields.io/badge/msrv-1.56.1-dea584.svg?logo=rust)](https://github.com/rust-lang/rust/releases/tag/1.56.1)
[![ci](https://github.com/allan2/dotenvy/actions/workflows/ci.yml/badge.svg)](https://github.com/allan2/dotenvy/actions/workflows/ci.yml)
[![docs](https://img.shields.io/docsrs/dotenvy?logo=docs.rs)](https://docs.rs/dotenvy/)

A well-maintained fork of the [dotenv](https://github.com/dotenv-rs/dotenv) crate.

This crate is the suggested alternative for `dotenv` in security advisory [RUSTSEC-2021-0141](https://rustsec.org/advisories/RUSTSEC-2021-0141.html).

This library loads environment variables from a _.env_ file. This is convenient for dev environments.

## Components

1. [`dotenvy`](https://crates.io/crates/dotenvy) crate - A well-maintained fork of the `dotenv` crate.
2. [`dotenvy_macro`](https://crates.io/crates/dotenvy_macro) crate - A macro for compile time dotenv inspection. This is a fork of `dotenv_codegen`.
3. `dotenvy` CLI tool for running a command using the environment from a _.env_ file (currently Unix only)

## Usage

### Loading at runtime

```rust
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables from .env file.
    // Fails if .env file not found, not readable or invalid.
    dotenvy::dotenv()?;

    for (key, value) in env::vars() {
        println!("{key}: {value}");
    }

    Ok(())
}
```

### Loading at compile time

The `dotenv!` macro provided by `dotenvy_macro` crate can be used.

Warning: there is an outstanding issue with rust-analyzer ([rust-analyzer #9606](https://github.com/rust-analyzer/rust-analyzer/issues/9606)) related to the `dotenv!` macro

## Minimum supported Rust version

Currently: **1.56.1**

We aim to support the latest 8 rustc versions - approximately 1 year. Increasing
MSRV is _not_ considered a semver-breaking change.

## Why does this fork exist?

The original dotenv crate has not been updated since June 26, 2020. Attempts to reach the authors and present maintainer were not successful ([dotenv-rs/dotenv #74](https://github.com/dotenv-rs/dotenv/issues/74)).

This fork intends to serve as the development home for the dotenv implementation in Rust.

## What are the differences from the original?

This repo fixes:

- more helpful errors for `dotenv!` ([dotenv-rs/dotenv #57](https://github.com/dotenv-rs/dotenv/pull/57))

It also adds:

- multiline support for environment variable values
- `io::Read` support via [`from_read`](https://docs.rs/dotenvy/latest/dotenvy/fn.from_read.html) and [`from_read_iter`](https://docs.rs/dotenvy/latest/dotenvy/fn.from_read_iter.html)
- override support via [`dotenv_override`], [`from_filename_override`], [`from_path_override`] and [`from_read_override`]
- improved docs

For a full list of changes, refer to the [changelog](./CHANGELOG.md).

## The legend

Legend has it that the Lost Maintainer will return, merging changes from `dotenvy` into `dotenv` with such thrust that all `Cargo.toml`s will lose one keystroke. Only then shall the Rust dotenv crateverse be united in true harmony.

Until then, this repo dutifully carries on the dotenv torch. It is actively maintained. Contributions and PRs are very welcome!
