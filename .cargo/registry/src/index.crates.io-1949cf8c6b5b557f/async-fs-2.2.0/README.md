# async-fs

[![CI](https://github.com/smol-rs/async-fs/actions/workflows/ci.yml/badge.svg)](
https://github.com/smol-rs/async-fs/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](
https://github.com/smol-rs/async-fs)
[![Cargo](https://img.shields.io/crates/v/async-fs.svg)](
https://crates.io/crates/async-fs)
[![Documentation](https://docs.rs/async-fs/badge.svg)](
https://docs.rs/async-fs)

Async filesystem primitives.

This crate is an async version of `std::fs`.

## Implementation

This crate uses [`blocking`] to offload blocking I/O onto a thread pool.

[`blocking`]: https://docs.rs/blocking

## Examples

Create a new file and write some bytes to it:

```rust
use async_fs::File;
use futures_lite::io::AsyncWriteExt;

let mut file = File::create("a.txt").await?;
file.write_all(b"Hello, world!").await?;
file.flush().await?;
```

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
