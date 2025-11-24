# tokio-rustls

[![github actions](https://github.com/rustls/tokio-rustls/workflows/CI/badge.svg)](https://github.com/rustls/tokio-rustls/actions)
[![crates](https://img.shields.io/crates/v/tokio-rustls.svg)](https://crates.io/crates/tokio-rustls)
[![license](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/rustls/tokio-rustls/blob/main/LICENSE-MIT)
[![license](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://github.com/rustls/tokio-rustls/blob/main/LICENSE-APACHE)
[![docs.rs](https://docs.rs/tokio-rustls/badge.svg)](https://docs.rs/tokio-rustls)

Asynchronous TLS/SSL streams for [Tokio](https://tokio.rs/) using
[Rustls](https://github.com/rustls/rustls).

### Basic Structure of a Client

```rust
use rustls_pki_types::ServerName;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

// ...

let mut root_cert_store = RootCertStore::empty();
root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
let config = ClientConfig::builder()
    .with_root_certificates(root_cert_store)
    .with_no_client_auth();
let connector = TlsConnector::from(Arc::new(config));
let dnsname = ServerName::try_from("www.rust-lang.org").unwrap();

let stream = TcpStream::connect(&addr).await?;
let mut stream = connector.connect(dnsname, stream).await?;

// ...
```

### Client Example Program

See [examples/client.rs](examples/client.rs). You can run it with:

```sh
cargo run --example client -- hsts.badssl.com
```

### Server Example Program

See [examples/server.rs](examples/server.rs). You can run it with:

```sh
cargo run --example server -- 127.0.0.1:8000 --cert mycert.der --key mykey.der
```

### License & Origin

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
  https://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  https://opensource.org/licenses/MIT)

at your option.

This started as a fork of [tokio-tls](https://github.com/tokio-rs/tokio-tls).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in tokio-rustls by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
