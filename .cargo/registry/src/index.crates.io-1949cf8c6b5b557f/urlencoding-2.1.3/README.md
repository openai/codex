# urlencoding

[![Latest Version](https://img.shields.io/crates/v/urlencoding.svg)](https://lib.rs/crates/urlencoding)

A tiny Rust library for doing URL percentage encoding and decoding. It percent-encodes everything except alphanumerics and `-`, `_`, `.`, `~`.

When decoding `+` is not treated as a space. Error recovery from incomplete percent-escapes follows the [WHATWG URL standard](https://url.spec.whatwg.org/).

## Usage

To encode a string, do the following:

```rust
use urlencoding::encode;

let encoded = encode("This string will be URL encoded.");
println!("{}", encoded);
// This%20string%20will%20be%20URL%20encoded.
```

To decode a string, it's only slightly different:

```rust
use urlencoding::decode;

let decoded = decode("%F0%9F%91%BE%20Exterminate%21")?;
println!("{}", decoded);
// 👾 Exterminate!
```

To decode allowing arbitrary bytes and invalid UTF-8:

```rust
use urlencoding::decode_binary;

let binary = decode_binary(b"%F1%F2%F3%C0%C1%C2");
let decoded = String::from_utf8_lossy(&binary);
```

This library returns [`Cow`](https://doc.rust-lang.org/stable/std/borrow/enum.Cow.html) to avoid allocating when decoding/encoding is not needed. Call `.into_owned()` on the `Cow` to get a `Vec` or `String`.

## License

This project is licensed under the MIT license. For more information see the `LICENSE` file.
