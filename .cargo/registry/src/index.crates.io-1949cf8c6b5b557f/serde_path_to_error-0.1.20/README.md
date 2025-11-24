# Serde path to error

[<img alt="github" src="https://img.shields.io/badge/github-dtolnay/path--to--error-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/dtolnay/path-to-error)
[<img alt="crates.io" src="https://img.shields.io/crates/v/serde_path_to_error.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/serde_path_to_error)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-serde__path__to__error-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/serde_path_to_error)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/dtolnay/path-to-error/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/dtolnay/path-to-error/actions?query=branch%3Amaster)

Find out the path at which a deserialization error occurred. This crate provides
a wrapper that works with any existing Serde `Deserializer` and exposes the
chain of field names leading to the error.

```toml
[dependencies]
serde = "1.0"
serde_path_to_error = "0.1"
```

```rust
use serde::Deserialize;
use std::collections::BTreeMap as Map;

#[derive(Deserialize)]
struct Package {
    name: String,
    dependencies: Map<String, Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    version: String,
}

fn main() {
    let j = r#"{
        "name": "demo",
        "dependencies": {
            "serde": {
                "version": 1
            }
        }
    }"#;

    // Some Deserializer.
    let jd = &mut serde_json::Deserializer::from_str(j);

    let result: Result<Package, _> = serde_path_to_error::deserialize(jd);
    match result {
        Ok(_) => panic!("expected a type error"),
        Err(err) => {
            let path = err.path().to_string();
            assert_eq!(path, "dependencies.serde.version");
        }
    }
}
```

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
