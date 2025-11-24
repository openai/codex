[![crates.io](https://img.shields.io/crates/v/hyper-timeout.svg)](https://crates.io/crates/hyper-timeout)

# hyper-timeout

A connect, read and write timeout aware connector to be used with hyper `Client`.

## Problem

At the time this crate was created, hyper did not support timeouts. There is a way to do general timeouts, but no easy way to get connect, read and write specific timeouts.

## Solution

There is a `TimeoutConnector` that implements the `hyper::Connect` trait. This connector wraps around `HttpConnector` or `HttpsConnector` values and provides timeouts.

> [!IMPORTANT]  
> The timeouts are on the underlying stream and _not_ the request.

- The read timeout will start when the underlying stream is first polled for read.
- The write timeout will start when the underlying stream is first polled for write.

Tokio often interleaves poll_read and poll_write calls to handle this bi-directional communication efficiently. Due to this behavior, both the read and write timeouts start at the same time. This means your read timeout can expire while the client is still writing the request to the server. If you are writing large bodies, consider using `set_reset_reader_on_write` to avoid this behavior.

## Usage

Hyper version compatibility:

- The `master` branch will track on going development for hyper.
- The `0.5` release supports hyper 1.0.
- The `0.4` release supports hyper 0.14.
- The `0.3` release supports hyper 0.13.
- The `0.2` release supports hyper 0.12.
- The `0.1` release supports hyper 0.11.
    - **Note:** In hyper 0.11, a read or write timeout will return a _broken pipe_ error because of the way `tokio_proto::ClientProto` works


Assuming you are using hyper 1.0, add this to your `Cargo.toml`:

```toml
[dependencies]
hyper-timeout = "0.5"
```

See the [client example](./examples/client.rs) for a working example.

## License

Licensed under either of
 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
