# rust-native-tls

[Documentation](https://docs.rs/native-tls)

An abstraction over platform-specific TLS implementations.

Specifically, this crate uses SChannel on Windows (via the [`schannel`] crate),
Secure Transport on macOS (via the [`security-framework`] crate), and OpenSSL (via
the [`openssl`] crate) on all other platforms.

[`schannel`]: https://crates.io/crates/schannel
[`security-framework`]: https://crates.io/crates/security-framework
[`openssl`]: https://crates.io/crates/openssl

## Installation

```toml
# Cargo.toml
[dependencies]
native-tls = "0.2"
```

## Usage

An example client looks like:

```rust,ignore
extern crate native_tls;

use native_tls::TlsConnector;
use std::io::{Read, Write};
use std::net::TcpStream;

fn main() {
    let connector = TlsConnector::new().unwrap();

    let stream = TcpStream::connect("google.com:443").unwrap();
    let mut stream = connector.connect("google.com", stream).unwrap();

    stream.write_all(b"GET / HTTP/1.0\r\n\r\n").unwrap();
    let mut res = vec![];
    stream.read_to_end(&mut res).unwrap();
    println!("{}", String::from_utf8_lossy(&res));
}
```

To accept connections as a server from remote clients:

```rust,ignore
extern crate native_tls;

use native_tls::{Identity, TlsAcceptor, TlsStream};
use std::fs::File;
use std::io::{Read};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

fn main() {
    let mut file = File::open("identity.pfx").unwrap();
    let mut identity = vec![];
    file.read_to_end(&mut identity).unwrap();
    let identity = Identity::from_pkcs12(&identity, "hunter2").unwrap();

    let acceptor = TlsAcceptor::new(identity).unwrap();
    let acceptor = Arc::new(acceptor);

    let listener = TcpListener::bind("0.0.0.0:8443").unwrap();

    fn handle_client(stream: TlsStream<TcpStream>) {
        // ...
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let acceptor = acceptor.clone();
                thread::spawn(move || {
                    let stream = acceptor.accept(stream).unwrap();
                    handle_client(stream);
                });
            }
            Err(e) => { /* connection failed */ }
        }
    }
}
```

# License

`rust-native-tls` is primarily distributed under the terms of both the MIT
license and the Apache License (Version 2.0), with portions covered by various
BSD-like licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
