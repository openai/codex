# Version 0.8.6

- Fix stack overflow when pushing large value to `Injector`. (#1146, #1147, #1159)

# Version 0.8.5

- Remove dependency on `cfg-if`. (#1072)

# Version 0.8.4

- Bump the minimum supported Rust version to 1.61. (#1037)

# Version 0.8.3

- Add `Stealer::{steal_batch_with_limit, steal_batch_with_limit_and_pop}` methods. (#903)
- Add `Injector::{steal_batch_with_limit, steal_batch_with_limit_and_pop}` methods. (#903)

# Version 0.8.2

- Bump the minimum supported Rust version to 1.38. (#877)

# Version 0.8.1

- Fix deque steal race condition. (#726)
- Add `Stealer::len` method. (#708)

# Version 0.8.0

**Note:** This release has been yanked. See [GHSA-pqqp-xmhj-wgcw](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-pqqp-xmhj-wgcw) for details.

- Bump the minimum supported Rust version to 1.36.
- Add `Worker::len()` and `Injector::len()` methods.
- Add `std` (enabled by default) feature for forward compatibility.

# Version 0.7.4

- Fix deque steal race condition.

# Version 0.7.3

**Note:** This release has been yanked. See [GHSA-pqqp-xmhj-wgcw](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-pqqp-xmhj-wgcw) for details.

- Stop stealing from the same deque. (#448)
- Fix unsoundness issues by adopting `MaybeUninit`. (#458)

# Version 0.7.2

**Note:** This release has been yanked. See [GHSA-pqqp-xmhj-wgcw](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-pqqp-xmhj-wgcw) for details.

- Bump `crossbeam-epoch` to `0.8`.
- Bump `crossbeam-utils` to `0.7`.

# Version 0.7.1

**Note:** This release has been yanked. See [GHSA-pqqp-xmhj-wgcw](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-pqqp-xmhj-wgcw) for details.

- Bump the minimum required version of `crossbeam-utils`.

# Version 0.7.0

**Note:** This release has been yanked. See [GHSA-pqqp-xmhj-wgcw](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-pqqp-xmhj-wgcw) for details.

- Make `Worker::pop()` faster in the FIFO case.
- Replace `fifo()` nad `lifo()` with `Worker::new_fifo()` and `Worker::new_lifo()`.
- Add more batched steal methods.
- Introduce `Injector<T>`, a MPMC queue.
- Rename `Steal::Data` to `Steal::Success`.
- Add `Steal::or_else()` and implement `FromIterator` for `Steal`.
- Add `#[must_use]` to `Steal`.

# Version 0.6.3

- Bump `crossbeam-epoch` to `0.7`.

# Version 0.6.2

- Update `crosbeam-utils` to `0.6`.

# Version 0.6.1

- Change a few `Relaxed` orderings to `Release` in order to fix false positives by tsan.

# Version 0.6.0

- Add `Stealer::steal_many` for batched stealing.
- Change the return type of `pop` to `Pop<T>` so that spinning can be handled manually.

# Version 0.5.2

- Update `crossbeam-utils` to `0.5.0`.

# Version 0.5.1

- Minor optimizations.

# Version 0.5.0

- Add two deque constructors : `fifo()` and `lifo()`.
- Update `rand` to `0.5.3`.
- Rename `Deque` to `Worker`.
- Return `Option<T>` from `Stealer::steal`.
- Remove methods `Deque::len` and `Stealer::len`.
- Remove method `Deque::stealer`.
- Remove method `Deque::steal`.

# Version 0.4.1

- Update `crossbeam-epoch` to `0.5.0`.

# Version 0.4.0

- Update `crossbeam-epoch` to `0.4.2`.
- Update `crossbeam-utils` to `0.4.0`.
- Require minimum Rust version 1.25.

# Version 0.3.1

- Add `Deque::capacity`.
- Add `Deque::min_capacity`.
- Add `Deque::shrink_to_fit`.
- Update `crossbeam-epoch` to `0.3.0`.
- Support Rust 1.20.
- Shrink the buffer in `Deque::push` if necessary.

# Version 0.3.0

- Update `crossbeam-epoch` to `0.4.0`.
- Drop support for Rust 1.13.

# Version 0.2.0

- Update `crossbeam-epoch` to `0.3.0`.
- Support Rust 1.13.

# Version 0.1.1

- Update `crossbeam-epoch` to `0.2.0`.

# Version 0.1.0

- First implementation of the Chase-Lev deque.
