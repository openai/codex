# Version 2.6.0

- Bump MSRV to 1.71. (#243)
- Expose `Timer::clear`. (#239)
- Implement `IoSafe` for `std::io::PipeReader` and `std::io::PipeWriter` (#237)
- Update to `windows-sys` v0.61. (#243)
- Remove dependency on `async_lock`. (#240)

# Version 2.5.0

- Add a new optional `tracing` feature. When enabled, this feature adds logging
  to the implementation. By default it is disabled. (#234)
- Add support for Haiku (#233)
- Fix build failure with minimal-versions. (#234)
- Update `windows-sys` to v0.60. (#230)

# Version 2.4.1

- Update to rustix version 1.0.7. (#221)

# Version 2.4.0

- Make it so the `Exit` filter can be created without passing ownership of the
  `Child` object. (#207)
- Add support for visionOS. (#202)
- Fix typo in documentation. (#204)

# Version 2.3.4

- Update `windows-sys` to v0.59. (#195)
- On NetBSD/DragonflyBSD, set `nosigpipe` on sockets. (#196)

# Version 2.3.3

- Fix nightly clippy warnings. (#191)

# Version 2.3.2

- Fix usage of the wrong socket flags on AIX. (#187)

# Version 2.3.1

- On Windows, call `WSAStartup` before any raw socket functions. (#183)

# Version 2.3.0

- Add `Waitable`, which allows waiting for waitable handles on
  Windows. (#152)

# Version 2.2.2

- Fix an `EINVAL` error that would occur when abstract sockets are used. (#176)

# Version 2.2.1

- Remove dependency on `waker-fn`. (#165)
- Update `windows-sys` to v0.52.0. (#173)

# Version 2.2.0

- Bump `async-lock` and `futures-lite` to their latest version. (#170)

# Version 2.1.0

- Implement `IoSafe` for `std::process::{ChildStdin, ChildStdout, ChildStderr}`. (#162)

# Version 2.0.0

- **Breaking:** `Async::new()` now takes types that implement `AsFd`/`AsSocket` instead of `AsRawFd`/`AsRawSocket`, in order to implement I/O safety. (#142)
- **Breaking:** `Async::get_mut()`, `Async::read_with_mut()` and `Async::write_with_mut()` are now `unsafe`. The underlying source is technically "borrowed" by the polling instance, so moving it out would be unsound. (#142)
- Expose miscellaneous `kqueue` filters in the `os::kqueue` module. (#112)
- Expose a way to get the underlying `Poller`'s file descriptor on Unix. (#125)
- Add a new `Async::new_nonblocking` method to allow users to avoid duplicating an already nonblocking socket. (#159)
- Remove the unused `fastrand` and `memchr` dependencies. (#131)
- Use `tracing` instead of `log`. (#140)
- Support ESP-IDF. (#144)
- Optimize the `block_on` function to reduce allocation, leading to a slight performance improvement. (#149)

# Version 1.13.0

- Use [`rustix`] instead of [`libc`]/[`windows-sys`] for system calls (#76)
- Add a `will_fire` method to `Timer` to test if it will ever fire (#106)
- Reduce syscalls in `Async::new` (#107)
- Improve the drop ergonomics of `Readable` and `Writable` (#109)
- Change the "`wepoll`" in documentation to "`IOCP`" (#116)

[`rustix`]: https://crates.io/crates/rustix/
[`libc`]: https://crates.io/crates/libc/
[`windows-sys`]: https://crates.io/crates/windows-sys/

# Version 1.12.0

- Switch from `winapi` to `windows-sys` (#102)

# Version 1.11.0

- Update `concurrent-queue` to v2. (#99)

# Version 1.10.0

- Remove the dependency on the `once_cell` crate to restore the MSRV. (#95)

# Version 1.9.0

- Fix panic on very large durations. (#87)
- Add `Timer::never` (#87)

# Version 1.8.0

- Implement I/O safety traits on Rust 1.63+ (#84)

# Version 1.7.0

- Process timers set for exactly `now`. (#73)

# Version 1.6.0

- Add `Readable` and `Writable` futures. (#64, #66)
- Add `Async::{readable_owned, writable_owned}`. (#66)

# Version 1.5.0 [YANKED]

- Add `Readable` and `Writable` futures. (#64)

# Version 1.4.1

- Remove dependency on deprecated `vec-arena`. (#60)

# Version 1.4.0

- Implement `AsRef<T>` and `AsMut<T>` for `Async<T>`. (#44)
- Remove dependency on deprecated `nb-connect`. (#55)

# Version 1.3.1

- Lower MSRV to 1.41.0

# Version 1.3.0

- Add `Timer::interval()` and `Timer::set_interval()`.
- Add `Timer::interval_at()` and `Timer::set_interval_at()`.
- Implement `Stream` for `Timer`.

# Version 1.2.0

- Add `Async::poll_readable()` and `Async::poll_writable()`.

# Version 1.1.10

- Update `futures-lite`.

# Version 1.1.9

- Only require `libc` on Unix platforms.

# Version 1.1.8

- Re-enable `async-net` dependency and fix CI.

# Version 1.1.7

- Update `polling` to v2.0.0

# Version 1.1.6

- Remove randomized yielding everywhere.

# Version 1.1.5

- Remove randomized yielding in write operations.

# Version 1.1.4

- Implement proper cancelation for `readable()` and `writable()`.

# Version 1.1.3

- Improve docs.

# Version 1.1.2

- Add `nb-connect` dependency.
- Remove `wepoll-sys-stjepang` dependency.

# Version 1.1.1

- Remove `socket2` dependency.

# Version 1.1.0

- Add `TryFrom` conversion impls for `Async`.

# Version 1.0.2

- Don't box `T` in `Async<T>`.
- `Async::incoming()` doesn't return `Unpin` streams anymore.

# Version 1.0.1

- Update dependencies.

# Version 1.0.0

- Stabilize.

# Version 0.2.7

- Replace `log::debug!` with `log::trace!`.

# Version 0.2.6

- Add logging.

# Version 0.2.5

- On Linux, fail fast if `writable()` succeeds after connecting to `UnixStream`,
  but the connection is not really established.

# Version 0.2.4

- Prevent threads in `async_io::block_on()` from hogging the reactor forever.

# Version 0.2.3

- Performance optimizations in `block_on()`.

# Version 0.2.2

- Add probabilistic yielding to improve fairness.

# Version 0.2.1

- Update readme.

# Version 0.2.0

- Replace `parking` module with `block_on()`.
- Fix a bug in `Async::<UnixStream>::connect()`.

# Version 0.1.11

- Bug fix: clear events list before polling.

# Version 0.1.10

- Simpler implementation of the `parking` module.
- Extracted raw bindings to epoll/kqueue/wepoll into the `polling` crate.

# Version 0.1.9

- Update dependencies.
- More documentation.

# Version 0.1.8

- Tweak the async-io to poll I/O less aggressively.

# Version 0.1.7

- Tweak the async-io thread to use less CPU.
- More examples.

# Version 0.1.6

- Add `Timer::reset()`.
- Add third party licenses.
- Code cleanup.

# Version 0.1.5

- Make `Parker` and `Unparker` unwind-safe.

# Version 0.1.4

- Initialize the reactor in `Parker::new()`.

# Version 0.1.3

- Always use the last waker given to `Timer`.
- Shutdown the socket in `AsyncWrite::poll_close()`.
- Reduce the number of dependencies.

# Version 0.1.2

- Shutdown the write side of the socket in `AsyncWrite::poll_close()`.
- Code and dependency cleanup.
- Always use the last waker when polling a timer.

# Version 0.1.1

- Initial version
