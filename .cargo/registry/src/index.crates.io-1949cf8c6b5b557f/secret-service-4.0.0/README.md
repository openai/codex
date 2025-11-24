# Secret Service

[![crates.io version](https://img.shields.io/crates/v/secret-service.svg)](https://crates.io/crates/secret-service)
[![crate documentation](https://docs.rs/secret-service/badge.svg)](https://docs.rs/secret-service)
![MSRV](https://img.shields.io/badge/rustc-1.75+-blue.svg)
[![crates.io downloads](https://img.shields.io/crates/d/secret-service.svg)](https://crates.io/crates/secret-service)
![CI](https://github.com/hwchen/secret-service-rs/workflows/CI/badge.svg)

A rust library for interacting with the FreeDesktop Secret Service API through DBus.

### Basic Usage

`secret-service` is implemented in pure Rust by default, so it doesn't require any system libraries
such as `libdbus-1-dev` or `libdbus-1-3` on Ubuntu.

In Cargo.toml:

When adding the crate, you must select a feature representing your selected runtime and cryptography backend. 
For example:

```toml
[dependencies]
secret-service = { version = "3.0.0", features = ["rt-tokio-crypto-rust"] }
```

Available feature flags:
- `rt-async-io-crypto-rust`: Uses the `async-std` runtime and pure Rust crytography via `RustCrypto`.
- `rt-async-io-crypto-openssl`: Uses the `async-std` runtime and OpenSSL as the cryptography provider.
- `rt-tokio-crypto-rust`: Uses the `tokio` runtime and pure Rust cryptography via `RustCrypto`.
- `rt-tokio-crypto-openssl`: Uses the `tokio` runtime and OpenSSL as the cryptography provider.

Note that the `-openssl` feature sets require OpenSSL to be available on your system, or the `bundled` feature
of `openssl` crate must be activated in your `cargo` dependency tree instead.

In source code (below example is for `--bin`, not `--lib`). This example uses `tokio` as
the async runtime.

```rust
use secret_service::SecretService;
use secret_service::EncryptionType;
use std::{collections::HashMap, error::Error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // initialize secret service (dbus connection and encryption session)
    let ss = SecretService::connect(EncryptionType::Dh).await?;

    // get default collection
    let collection = ss.get_default_collection().await?;

    // create new item
    collection.create_item(
        "test_label", // label
        HashMap::from([("test", "test_value")]), // properties
        b"test_secret", // secret
        false, // replace item with same attributes
        "text/plain" // secret content type
    ).await?;

    // search items by properties
    let search_items = ss.search_items(
        HashMap::from([("test", "test_value")])
    ).await?;

    let item = search_items.unlocked.first().ok_or("Not found!")?;

    // retrieve secret from item
    let secret = item.get_secret().await?;
    assert_eq!(secret, b"test_secret");

    // delete item (deletes the dbus object, not the struct instance)
    item.delete().await?;
    Ok(())
}
```

### Functionality

- SecretService: initialize dbus, create plain/encrypted session.
- Collections: create, delete, search.
- Items: create, delete, search, get/set secret.

### Changelog
See [the list of GitHub releases and their release notes](https://github.com/hwchen/secret-service-rs/releases)

### Versioning
This library is feature complete, has stabilized its API for the most part. However, as this
crate is almost soley reliable on the `zbus` crate, we try and match major version releases
with theirs to handle breaking changes and move with the wider `zbus` ecosystem.

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
