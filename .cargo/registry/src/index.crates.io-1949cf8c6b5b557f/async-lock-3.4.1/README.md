# async-lock

[![Build](https://github.com/smol-rs/async-lock/workflows/Build%20and%20test/badge.svg)](
https://github.com/smol-rs/async-lock/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](
https://github.com/smol-rs/async-lock)
[![Cargo](https://img.shields.io/crates/v/async-lock.svg)](
https://crates.io/crates/async-lock)
[![Documentation](https://docs.rs/async-lock/badge.svg)](
https://docs.rs/async-lock)

Async synchronization primitives.

This crate provides the following primitives:

* `Barrier` - enables tasks to synchronize all together at the same time.
* `Mutex` - a mutual exclusion lock.
* `RwLock` - a reader-writer lock, allowing any number of readers or a single writer.
* `Semaphore` - limits the number of concurrent operations.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
