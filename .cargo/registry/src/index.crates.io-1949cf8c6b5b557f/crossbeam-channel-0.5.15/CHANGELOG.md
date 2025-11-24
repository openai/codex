# Version 0.5.15

- Fix regression introduced in 0.5.12 that can lead to a double free when dropping unbounded channel. (#1187)

# Version 0.5.14

**Note:** This release has been yanked due to bug fixed in 0.5.15.

- Fix stack overflow when sending large value to unbounded channel. (#1146, #1147)
- Add `Select::new_biased` function. (#1150)
- Remove inefficient spinning. (#1154)
- Suppress buggy `clippy::zero_repeat_side_effects` lint in macro generated code. (#1123)

# Version 0.5.13

**Note:** This release has been yanked due to bug fixed in 0.5.15.

- Add `select_biased!` macro. (#1040)

# Version 0.5.12

**Note:** This release has been yanked due to bug fixed in 0.5.15.

- Fix memory leak in unbounded channel. (#1084)

# Version 0.5.11

- Remove dependency on `cfg-if`. (#1072)

# Version 0.5.10

- Relax the minimum supported Rust version to 1.60. (#1056)
- Optimize `Drop` implementation of bounded channel. (#1057)

# Version 0.5.9

- Bump the minimum supported Rust version to 1.61. (#1037)

# Version 0.5.8

- Fix race condition in unbounded channel. (#972)

# Version 0.5.7

**Note:** This release has been yanked due to bug fixed in 0.5.8.

- Improve handling of very large timeout. (#953)

# Version 0.5.6

**Note:** This release has been yanked due to bug fixed in 0.5.8.

- Bump the minimum supported Rust version to 1.38. (#877)

# Version 0.5.5

**Note:** This release has been yanked due to bug fixed in 0.5.8.

- Replace Spinlock with Mutex. (#835)

# Version 0.5.4

**Note:** This release has been yanked due to bug fixed in 0.5.8.

- Workaround a bug in upstream related to TLS access on AArch64 Linux. (#802)

# Version 0.5.3

**Note:** This release has been yanked. See [#802](https://github.com/crossbeam-rs/crossbeam/issues/802) for details.

- Fix panic on very large timeout. (#798)

# Version 0.5.2

**Note:** This release has been yanked. See [#802](https://github.com/crossbeam-rs/crossbeam/issues/802) for details.

- Fix stacked borrows violations when `-Zmiri-tag-raw-pointers` is enabled. (#763, #764)

# Version 0.5.1

**Note:** This release has been yanked due to bug fixed in 0.5.8.

- Fix memory leak in unbounded channel. (#669)

# Version 0.5.0

- Bump the minimum supported Rust version to 1.36.
- Add `at()` function.
- Add `Sender::send_deadline()` and `Receiver::recv_deadline()` methods.
- Add `Select::select_deadline()` and `Select::ready_deadline()` methods.
- Add `std` (enabled by default) feature for forward compatibility.
- Allow `select!` macro compile with `forbid(unsafe_code)`.

# Version 0.4.4

- Fix bug in release (yanking 0.4.3)
- Fix UB and breaking change introduced in 0.4.3

# Version 0.4.3

**Note:** This release has been yanked. See [GHSA-v5m7-53cv-f3hx](https://github.com/crossbeam-rs/crossbeam/security/advisories/GHSA-v5m7-53cv-f3hx) for details.

- Change license to "MIT OR Apache-2.0".

# Version 0.4.2

- Fix bug in release (yanking 0.4.1)

# Version 0.4.1

- Avoid time drift in `channel::tick`. (#456)
- Fix unsoundness issues by adopting `MaybeUninit`. (#458)

# Version 0.4.0

- Bump the minimum required version to 1.28.
- Bump `crossbeam-utils` to `0.7`.

# Version 0.3.9

- Fix a bug in reference counting.
- Optimize `recv_timeout()`.
- Add `Select::remove()`.
- Various small improvements, code cleanup, more tests.

# Version 0.3.8

- Bump the minimum required version of `crossbeam-utils`.

# Version 0.3.7

- Remove `parking_lot` and `rand` dependencies.
- Expand documentation.
- Implement `Default` for `Select`.
- Make `size_of::<Receiver<T>>()` smaller.
- Several minor optimizations.
- Add more tests.

# Version 0.3.6

- Fix a bug in initialization of unbounded channels.

# Version 0.3.5

- New implementation for unbounded channels.
- A number of small performance improvements.
- Remove `crossbeam-epoch` dependency.

# Version 0.3.4

- Bump `crossbeam-epoch` to `0.7`.
- Improve documentation.

# Version 0.3.3

- Relax the lifetime in `SelectedOperation<'_>`.
- Add `Select::try_ready()`, `Select::ready()`, and `Select::ready_timeout()`.
- Update licensing notices.
- Improve documentation.
- Add methods `is_disconnected()`, `is_timeout()`, `is_empty()`, and `is_full()` on error types.

# Version 0.3.2

- More elaborate licensing notices.

# Version 0.3.1

- Update `crossbeam-utils` to `0.6`.

# Version 0.3.0

- Add a special `never` channel type.
- Dropping all receivers now closes the channel.
- The interface of sending and receiving methods is now very similar to those in v0.1.
- The syntax for `send` in `select!` is now `send(sender, msg) -> res => body`.
- The syntax for `recv` in `select!` is now `recv(receiver) -> res => body`.
- New, more efficient interface for `Select` without callbacks.
- Timeouts can be specified in `select!`.

# Version 0.2.6

- `Select` struct that can add cases dynamically.
- More documentation (in particular, the FAQ section).
- Optimize contended sends/receives in unbounded channels.

# Version 0.2.5

- Use `LocalKey::try_with` instead of `LocalKey::with`.
- Remove helper macros `__crossbeam_channel*`.

# Version 0.2.4

- Make `select!` linearizable with other channel operations.
- Update `crossbeam-utils` to `0.5.0`.
- Update `parking_lot` to `0.6.3`.
- Remove Mac OS X tests.

# Version 0.2.3

- Add Mac OS X tests.
- Lower some memory orderings.
- Eliminate calls to `mem::unitialized`, which caused bugs with ZST.

# Version 0.2.2

- Add more tests.
- Update `crossbeam-epoch` to 0.5.0
- Initialize the RNG seed to a random value.
- Replace `libc::abort` with `std::process::abort`.
- Ignore clippy warnings in `select!`.
- Better interaction of `select!` with the NLL borrow checker.

# Version 0.2.1

- Fix compilation errors when using `select!` with `#[deny(unsafe_code)]`.

# Version 0.2.0

- Implement `IntoIterator<Item = T>` for `Receiver<T>`.
- Add a new `select!` macro.
- Add special channels `after` and `tick`.
- Dropping receivers doesn't close the channel anymore.
- Change the signature of `recv`, `send`, and `try_recv`.
- Remove `Sender::is_closed` and `Receiver::is_closed`.
- Remove `Sender::close` and `Receiver::close`.
- Remove `Sender::send_timeout` and `Receiver::recv_timeout`.
- Remove `Sender::try_send`.
- Remove `Select` and `select_loop!`.
- Remove all error types.
- Remove `Iter`, `TryIter`, and `IntoIter`.
- Remove the `nightly` feature.
- Remove ordering operators for `Sender` and `Receiver`.

# Version 0.1.3

- Add `Sender::disconnect` and `Receiver::disconnect`.
- Implement comparison operators for `Sender` and `Receiver`.
- Allow arbitrary patterns in place of `msg` in `recv(r, msg)`.
- Add a few conversion impls between error types.
- Add benchmarks for `atomicring` and `mpmc`.
- Add benchmarks for different message sizes.
- Documentation improvements.
- Update `crossbeam-epoch` to 0.4.0
- Update `crossbeam-utils` to 0.3.0
- Update `parking_lot` to 0.5
- Update `rand` to 0.4

# Version 0.1.2

- Allow conditional cases in `select_loop!` macro.
- Fix typos in documentation.
- Fix deadlock in selection when all channels are disconnected and a timeout is specified.

# Version 0.1.1

- Implement `Debug` for `Sender`, `Receiver`, `Iter`, `TryIter`, `IntoIter`, and `Select`.
- Implement `Default` for `Select`.

# Version 0.1.0

- First implementation of the channels.
- Add `select_loop!` macro by @TimNN.
