env_flags
=========
[<img alt="github" src="https://img.shields.io/badge/github-kykosic/env--flags-a68bbd?style=for-the-badge&logo=github" height="20">](https://github.com/kykosic/env-flags)
[<img alt="crates.io" src="https://img.shields.io/crates/v/env-flags?style=for-the-badge&color=f0963a&logo=rust" height="20">](https://crates.io/crates/env-flags)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-env--flags-57979e?style=for-the-badge&logo=docs.rs" height="20">](https://docs.rs/env-flags)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/kykosic/env-flags/ci.yml?branch=main&style=for-the-badge" height="20">](https://github.com/kykosic/env-flags/actions?query=branch%3Amain)

This library provides a convenient macro for declaring environment variables.

```toml
[dependencies]
env-flags = "0.1"
```

_Compiler support: requires rustc 1.80+_

## Example

```rust
use env_flags::env_flags;

use std::time::Duration;

env_flags! {
    /// Required env var, panics if missing.
    AUTH_TOKEN: &str;
    /// Env var with a default value if not specified.
    pub(crate) PORT: u16 = 8080;
    /// An optional env var.
    pub OVERRIDE_HOSTNAME: Option<&str> = None;

    /// `Duration` by default is parsed as `f64` seconds.
    TIMEOUT: Duration = Duration::from_secs(5);
    /// Custom parsing function, takes a `String` and returns a `Result<Duration>`.
    TIMEOUT_MS: Duration = Duration::from_millis(30), |value| {
        value.parse().map(Duration::from_millis)
    };

    /// `bool` can be true, false, 1, or 0 (case insensitive)
    /// eg. export ENABLE_FEATURE="true"
    pub ENABLE_FEATURE: bool = true;

    /// `Vec<T>` by default is parsed as a comma-seprated string
    /// eg. export VALID_PORTS="80,443,9121"
    pub VALID_PORTS: Vec<u16> = vec![80, 443, 9121];
}
```
