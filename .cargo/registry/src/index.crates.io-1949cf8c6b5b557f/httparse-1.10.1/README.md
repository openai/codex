# httparse

[![crates.io](https://img.shields.io/crates/v/httparse.svg)](https://crates.io/crates/httparse)
[![Released API docs](https://docs.rs/httparse/badge.svg)](https://docs.rs/httparse)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE-MIT)
[![CI](https://github.com/seanmonstar/httparse/workflows/CI/badge.svg)](https://github.com/seanmonstar/httparse/actions?query=workflow%3ACI)
[![Discord chat][discord-badge]][discord-url]

A push parser for the HTTP 1.x protocol. Avoids allocations. No copy. **Fast.**

Works with `no_std`, simply disable the `std` Cargo feature.

[Changelog](https://github.com/seanmonstar/httparse/releases)


[discord-badge]: https://img.shields.io/discord/500028886025895936.svg?logo=discord
[discord-url]: https://discord.gg/kkwpueZ

## Usage

```rust
let mut headers = [httparse::EMPTY_HEADER; 64];
let mut req = httparse::Request::new(&mut headers);

let buf = b"GET /index.html HTTP/1.1\r\nHost";
assert!(req.parse(buf)?.is_partial());

// a partial request, so we try again once we have more data

let buf = b"GET /index.html HTTP/1.1\r\nHost: example.domain\r\n\r\n";
assert!(req.parse(buf)?.is_complete());
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or https://apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
