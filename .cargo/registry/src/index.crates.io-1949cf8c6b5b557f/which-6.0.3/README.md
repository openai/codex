[![Build Status](https://github.com/harryfei/which-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/harryfei/which-rs/actions/workflows/rust.yml)

# which

A Rust equivalent of Unix command "which". Locate installed executable in cross platforms.

## Support platforms

* Linux
* Windows
* macOS

## Examples

1) To find which rustc executable binary is using.

    ``` rust
    use which::which;

    let result = which("rustc").unwrap();
    assert_eq!(result, PathBuf::from("/usr/bin/rustc"));
    ```

2. After enabling the `regex` feature, find all cargo subcommand executables on the path:

    ``` rust
    use which::which_re;

    which_re(Regex::new("^cargo-.*").unwrap()).unwrap()
        .for_each(|pth| println!("{}", pth.to_string_lossy()));
    ```

## MSRV

This crate currently has an MSRV of Rust 1.70. Increasing the MSRV is considered a breaking change and thus requires a major version bump.

We cannot make any guarantees about the MSRV of our dependencies. You may be required to pin one of our dependencies to a lower version in your own Cargo.toml in order to compile
with the minimum supported Rust version. Eventually Cargo will handle this automatically. See [rust-lang/cargo#9930](https://github.com/rust-lang/cargo/issues/9930) for more.

## Documentation

The documentation is [available online](https://docs.rs/which/).
