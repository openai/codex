parking_lot
============

[![Rust](https://github.com/Amanieu/parking_lot/workflows/Rust/badge.svg)](https://github.com/Amanieu/parking_lot/actions)
[![Crates.io](https://img.shields.io/crates/v/parking_lot.svg)](https://crates.io/crates/parking_lot)

[Documentation (synchronization primitives)](https://docs.rs/parking_lot/)

[Documentation (core parking lot API)](https://docs.rs/parking_lot_core/)

[Documentation (type-safe lock API)](https://docs.rs/lock_api/)

This library provides implementations of `Mutex`, `RwLock`, `Condvar` and
`Once` that are smaller, faster and more flexible than those in the Rust
standard library, as well as a `ReentrantMutex` type which supports recursive
locking. It also exposes a low-level API for creating your own efficient
synchronization primitives.

When tested on x86_64 Linux, `parking_lot::Mutex` was found to be 1.5x
faster than `std::sync::Mutex` when uncontended, and up to 5x faster when
contended from multiple threads. The numbers for `RwLock` vary depending on
the number of reader and writer threads, but are almost always faster than
the standard library `RwLock`, and even up to 50x faster in some cases.

## Features

The primitives provided by this library have several advantages over those
in the Rust standard library:

1. `Mutex` and `Once` only require 1 byte of storage space, while `Condvar`
   and `RwLock` only require 1 word of storage space. On the other hand on
   some platforms (macOS and a few others) the standard library primitives
   require a dynamically allocated `Box` to hold OS-specific synchronization 
   primitives. The small size of `Mutex` in particular encourages the use
   of fine-grained locks to increase parallelism.
2. Uncontended lock acquisition and release is done through fast inline
   paths which only require a single atomic operation.
3. Microcontention (a contended lock with a short critical section) is
   efficiently handled by spinning a few times while trying to acquire a
   lock.
4. The locks are adaptive and will suspend a thread after a few failed spin
   attempts. This makes the locks suitable for both long and short critical
   sections.
5. `Condvar`, `RwLock` and `Once` work on Windows XP, unlike the standard
   library versions of those types.
6. `RwLock` takes advantage of hardware lock elision on processors that
   support it, which can lead to huge performance wins with many readers.
   This must be enabled with the `hardware-lock-elision` feature.
7. `RwLock` uses a task-fair locking policy, which avoids reader and writer
   starvation, whereas the standard library version makes no guarantees.
8. `Condvar` is guaranteed not to produce spurious wakeups. A thread will
    only be woken up if it timed out or it was woken up by a notification.
9. `Condvar::notify_all` will only wake up a single thread and requeue the
    rest to wait on the associated `Mutex`. This avoids a thundering herd
    problem where all threads try to acquire the lock at the same time.
10. `RwLock` supports atomically downgrading a write lock into a read lock.
11. `Mutex` and `RwLock` allow raw unlocking without a RAII guard object.
12. `Mutex<()>` and `RwLock<()>` allow raw locking without a RAII guard
    object.
13. `Mutex` and `RwLock` support [eventual fairness](https://trac.webkit.org/changeset/203350)
    which allows them to be fair on average without sacrificing performance.
14. A `ReentrantMutex` type which supports recursive locking.
15. An *experimental* deadlock detector that works for `Mutex`,
    `RwLock` and `ReentrantMutex`. This feature is disabled by default and
    can be enabled via the `deadlock_detection` feature.
16. `RwLock` supports atomically upgrading an "upgradable" read lock into a
    write lock.
17. Optional support for [serde](https://docs.serde.rs/serde/).  Enable via the
    feature `serde`.  **NOTE!** this support is for `Mutex`, `ReentrantMutex`,
    and `RwLock` only; `Condvar` and `Once` are not currently supported.
18. Lock guards can be sent to other threads when the `send_guard` feature is
    enabled.

## The parking lot

To keep these primitives small, all thread queuing and suspending
functionality is offloaded to the *parking lot*. The idea behind this is
based on the Webkit [`WTF::ParkingLot`](https://webkit.org/blog/6161/locking-in-webkit/)
class, which essentially consists of a hash table mapping of lock addresses
to queues of parked (sleeping) threads. The Webkit parking lot was itself
inspired by Linux [futexes](https://man7.org/linux/man-pages/man2/futex.2.html),
but it is more powerful since it allows invoking callbacks while holding a queue
lock.

## Nightly vs stable

There are a few restrictions when using this library on stable Rust:

- The `wasm32-unknown-unknown` target is only fully supported on nightly with
  `-C target-feature=+atomics` in `RUSTFLAGS` and `-Zbuild-std=panic_abort,std`
  passed to cargo. parking_lot will work mostly fine on stable, the only
  difference is it will panic instead of block forever if you hit a deadlock.
  Just make sure not to enable `-C target-feature=+atomics` on stable as that
  will allow wasm to run with multiple threads which will completely break
  parking_lot's concurrency guarantees.

To enable nightly-only functionality, you need to enable the `nightly` feature
in Cargo (see below).

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
parking_lot = "0.12"
```

To enable nightly-only features, add this to your `Cargo.toml` instead:

```toml
[dependencies]
parking_lot = { version = "0.12", features = ["nightly"] }
```

The experimental deadlock detector can be enabled with the
`deadlock_detection` Cargo feature.

To allow sending `MutexGuard`s and `RwLock*Guard`s to other threads, enable the
`send_guard` option.

Note that the `deadlock_detection` and `send_guard` features are incompatible
and cannot be used together.

Hardware lock elision support for x86 can be enabled with the
`hardware-lock-elision` feature. This requires Rust 1.59 due to the use of
inline assembly.

The core parking lot API is provided by the `parking_lot_core` crate. It is
separate from the synchronization primitives in the `parking_lot` crate so that
changes to the core API do not cause breaking changes for users of `parking_lot`.

## Minimum Rust version

The current minimum required Rust version is 1.64. Any change to this is
considered a breaking change and will require a major version bump.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
