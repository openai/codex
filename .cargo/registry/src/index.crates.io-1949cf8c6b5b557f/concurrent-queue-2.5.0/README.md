# concurrent-queue

[![Build](https://github.com/smol-rs/concurrent-queue/workflows/Build%20and%20test/badge.svg)](
https://github.com/smol-rs/concurrent-queue/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](
https://github.com/smol-rs/concurrent-queue)
[![Cargo](https://img.shields.io/crates/v/concurrent-queue.svg)](
https://crates.io/crates/concurrent-queue)
[![Documentation](https://docs.rs/concurrent-queue/badge.svg)](
https://docs.rs/concurrent-queue)

A concurrent multi-producer multi-consumer queue.

There are two kinds of queues:

1. Bounded queue with limited capacity.
2. Unbounded queue with unlimited capacity.

Queues also have the capability to get closed at any point. When closed, no more items can be
pushed into the queue, although the remaining items can still be popped.

These features make it easy to build channels similar to `std::sync::mpsc` on top of this
crate.

## Examples

```rust
use concurrent_queue::ConcurrentQueue;

let q = ConcurrentQueue::unbounded();
q.push(1).unwrap();
q.push(2).unwrap();

assert_eq!(q.pop(), Ok(1));
assert_eq!(q.pop(), Ok(2));
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
