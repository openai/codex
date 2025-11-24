`x-www-form-urlencoded` meets Serde
===================================

This crate is a Rust library for serialising to and deserialising from
the [`application/x-www-form-urlencoded`][urlencoded] format. It is built
upon [Serde], a high performance generic serialization framework and [rust-url],
a URL parser for Rust.

[rust-url]: https://github.com/servo/rust-url
[Serde]: https://github.com/serde-rs/serde
[urlencoded]: https://url.spec.whatwg.org/#application/x-www-form-urlencoded

Installation
============

This crate works with Cargo and can be found on
[crates.io] with a `Cargo.toml` like:

```toml
[dependencies]
serde_urlencoded = "0.7"
```

The documentation is available on [docs.rs].

[crates.io]: https://crates.io/crates/serde_urlencoded
[docs.rs]: https://docs.rs/serde_urlencoded/0.7.1/serde_urlencoded/

## Getting help

Serde developers live in the #serde channel on
[`irc.mozilla.org`](https://wiki.mozilla.org/IRC) and most rust-url developers
live in the #servo one. The #rust channel is also a good resource with generally
faster response time but less specific knowledge about Serde, rust-url or this
crate. If IRC is not your thing, we are happy to respond to [GitHub
issues](https://github.com/nox/serde_urlencoded/issues/new) as well.

## License

serde_urlencoded is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in serde_urlencoded by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
