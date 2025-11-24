# Crossbeam Utils

[![Build Status](https://github.com/crossbeam-rs/crossbeam/workflows/CI/badge.svg)](
https://github.com/crossbeam-rs/crossbeam/actions)
[![License](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-blue.svg)](
https://github.com/crossbeam-rs/crossbeam/tree/master/crossbeam-utils#license)
[![Cargo](https://img.shields.io/crates/v/crossbeam-utils.svg)](
https://crates.io/crates/crossbeam-utils)
[![Documentation](https://docs.rs/crossbeam-utils/badge.svg)](
https://docs.rs/crossbeam-utils)
[![Rust 1.60+](https://img.shields.io/badge/rust-1.60+-lightgray.svg)](
https://www.rust-lang.org)
[![chat](https://img.shields.io/discord/569610676205781012.svg?logo=discord)](https://discord.com/invite/JXYwgWZ)

This crate provides miscellaneous tools for concurrent programming:

#### Atomics

* [`AtomicCell`], a thread-safe mutable memory location.<sup>(no_std)</sup>
* [`AtomicConsume`], for reading from primitive atomic types with "consume" ordering.<sup>(no_std)</sup>

#### Thread synchronization

* [`Parker`], a thread parking primitive.
* [`ShardedLock`], a sharded reader-writer lock with fast concurrent reads.
* [`WaitGroup`], for synchronizing the beginning or end of some computation.

#### Utilities

* [`Backoff`], for exponential backoff in spin loops.<sup>(no_std)</sup>
* [`CachePadded`], for padding and aligning a value to the length of a cache line.<sup>(no_std)</sup>
* [`scope`], for spawning threads that borrow local variables from the stack.

*Features marked with <sup>(no_std)</sup> can be used in `no_std` environments.*<br/>

[`AtomicCell`]: https://docs.rs/crossbeam-utils/*/crossbeam_utils/atomic/struct.AtomicCell.html
[`AtomicConsume`]: https://docs.rs/crossbeam-utils/*/crossbeam_utils/atomic/trait.AtomicConsume.html
[`Parker`]: https://docs.rs/crossbeam-utils/*/crossbeam_utils/sync/struct.Parker.html
[`ShardedLock`]: https://docs.rs/crossbeam-utils/*/crossbeam_utils/sync/struct.ShardedLock.html
[`WaitGroup`]: https://docs.rs/crossbeam-utils/*/crossbeam_utils/sync/struct.WaitGroup.html
[`Backoff`]: https://docs.rs/crossbeam-utils/*/crossbeam_utils/struct.Backoff.html
[`CachePadded`]: https://docs.rs/crossbeam-utils/*/crossbeam_utils/struct.CachePadded.html
[`scope`]: https://docs.rs/crossbeam-utils/*/crossbeam_utils/thread/fn.scope.html

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
crossbeam-utils = "0.8"
```

## Compatibility

Crossbeam Utils supports stable Rust releases going back at least six months,
and every time the minimum supported Rust version is increased, a new minor
version is released. Currently, the minimum supported Rust version is 1.60.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
