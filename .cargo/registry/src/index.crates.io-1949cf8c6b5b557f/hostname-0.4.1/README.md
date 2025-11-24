# hostname

[![Latest Version](https://img.shields.io/crates/v/hostname.svg)](https://crates.io/crates/hostname)
[![Latest Version](https://docs.rs/hostname/badge.svg)](https://docs.rs/hostname)
[![Build Status](https://github.com/svartalf/hostname/actions/workflows/ci.yml/badge.svg)](https://github.com/svartalf/hostname/actions)
![Minimum rustc version](https://img.shields.io/badge/rustc-1.71+-green.svg)
![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)
![Crates.io](https://img.shields.io/crates/d/hostname)

> Cross-platform system's host name functions in Rust

## Supported platforms

-   [POSIX-compliant](https://en.wikipedia.org/wiki/POSIX#POSIX-oriented_operating_systems) systems\
    (Linux, macOS, Android, FreeBSD, OpenBSD, NetBSD, Solaris, Redox, and so on)
-   Windows

## Rust version requirements

Since version `0.4.0` this crate requires Rust version `1.74` or greater.

This version is explicitly tested in CI
and may be bumped in any major or minor release as needed.\
Maintaining compatibility with older compilers is a priority though,
so the bar for bumping the minimum supported version is set very high.
Any changes to the supported minimum version will be called out in the release notes.

## Usage

Add the following dependency to your Cargo manifest:

```toml
[dependencies]
hostname = "^0.4"
```

Crate API provides two simple functions for retrieving and setting the system's host name:

```rust
use std::io;

fn main() -> io::Result<()> {
    // Retrieve the hostname
    dbg!(hostname::get()?);

    // And set a new one
    hostname::set("potato")?;

    Ok(())
}
```

## License

hostname is primarily distributed under the terms of the MIT license
([LICENSE](LICENSE) or http://opensource.org/licenses/MIT).
