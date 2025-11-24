# 1.0.4

* Update windows-sys to 0.59
  (https://github.com/tokio-rs/mio/pull/1857).
* Fix build failure of uds tests on Android
  (https://github.com/tokio-rs/mio/pull/1847).
* Fix set nonblocking socket on AIX
  (https://github.com/tokio-rs/mio/pull/1867).
* Emit a better error when using mio on WASM
  (https://github.com/tokio-rs/mio/pull/1856).

# 1.0.3

* Implement more I/O safety traits
  (https://github.com/tokio-rs/mio/pull/1831).
* Remove hermit-abi dependency, now using libc
  (https://github.com/tokio-rs/mio/pull/1830).
* Use `poll(2)` implementation on AIX, removing the need for using
  `mio_unsupported_force_poll_poll`
  (https://github.com/tokio-rs/mio/pull/1833).

# 1.0.2

* Work around eventfd bug on illumos
  (https://github.com/tokio-rs/mio/pull/1826).
* Fix dead code warning on Hermit when building with `--no-default-features
  --features os-ext`
  (https://github.com/tokio-rs/mio/pull/1828).

# 1.0.1

* Added Fuchsia support
  (https://github.com/tokio-rs/mio/pull/1811).
* Added GNU/Hurd support
  (https://github.com/tokio-rs/mio/pull/1816).
* Fixed an issue where accepting on a UDS socket without sometime pass an address
  with a NULL byte to SocketAddr::from_pathname
  (https://github.com/tokio-rs/mio/pull/1817).
* Internal cleanups that should make the `cfg` situations easier to follow
  (https://github.com/tokio-rs/mio/pull/1812,
  https://github.com/tokio-rs/mio/pull/1813).

# 1.0

With v1 Mio is able to bump its MSRV to 1.70, allowing us to implement I/O
safety traits (https://github.com/rust-lang/rust/issues/87074) and replace
`SocketAddr` with the version found in the standard library.

## Added

* Implement `AsFd` for`TcpListener`, `TcpStream`, `UdpSocket`, `UnixDatagram`,
  `UnixListener`, `UnixStream`, `pipe::Receiver` and `pipe::Sender`
  (https://github.com/tokio-rs/mio/pull/1749, https://github.com/tokio-rs/mio/pull/1797).
* Implement `From` for `TcpListener`, `TcpStream`, `UdpSocket`, `UnixDatagram`,
  `UnixListener`, and `UnixStream` for their standard library counterpart
  (https://github.com/tokio-rs/mio/pull/1767).
* Add support for abstract namespaces on Android
  (https://github.com/tokio-rs/mio/pull/1749).
* Add support for QNX OS
  (https://github.com/tokio-rs/mio/pull/1766,
  https://github.com/tokio-rs/mio/pull/1800).
* Add support for Apple visionOS
  (https://github.com/tokio-rs/mio/pull/1795).
* Support for Haiku
  (https://github.com/tokio-rs/mio/pull/1807).

## Removed

* The `SocketAddr` type is removed in favour of `std::os::unix::net::SocketAddr`
  (https://github.com/tokio-rs/mio/pull/1760). All methods on Mio's version
  should exist on the version in the standard library.

## Changes

* MSRV was updated to 1.74, updating to Rust edition edition
  (https://github.com/tokio-rs/mio/pull/1733).
* `UnixDatagram::{local_addr,peer_addr,bind_addr,recv_from}`,
  `UnixListener::{local_addr,bind_addr,accept}` and
  `UnixStream::{local_addr,peer_addr,connect_addr}` return and/or use
  `std::os::unix::net::SocketAddr` instead of Mio's own `SocketAddr` type
  (https://github.com/tokio-rs/mio/pull/1760).
* Use `OwnedFd` internally for `Poll` where possible
  (https://github.com/tokio-rs/mio/pull/1749).
* Support ESP-IDF and Hermit without cfg flags
  (https://github.com/tokio-rs/mio/pull/1789,
  https://github.com/tokio-rs/mio/pull/1802,
  https://github.com/tokio-rs/mio/pull/1802).
* Updated windows-sys to v0.52
  (https://github.com/tokio-rs/mio/pull/1668).

# 0.8.11

* Fix receiving IOCP events after deregistering a Windows named pipe
  (https://github.com/tokio-rs/mio/pull/1760, backport pr:
  https://github.com/tokio-rs/mio/pull/1761).

# 0.8.10

## Added

* Solaris support
  (https://github.com/tokio-rs/mio/pull/1724).

# 0.8.9

## Added

* ESP-IDF framework support
  (https://github.com/tokio-rs/mio/pull/1692).
* AIX operating system support
  (https://github.com/tokio-rs/mio/pull/1704).
* Vita support
  (https://github.com/tokio-rs/mio/pull/1721).
* `{UnixListener,UnixStream}:bind_addr`
  (https://github.com/tokio-rs/mio/pull/1630).
* `mio_unsupported_force_poll_poll` and `mio_unsupported_force_waker_pipe`
  **unsupported** configuration flags to force a specific poll or waker
  implementation
  (https://github.com/tokio-rs/mio/pull/1684,
  https://github.com/tokio-rs/mio/pull/1685,
  https://github.com/tokio-rs/mio/pull/1692).

## Fixed

* The `pipe(2)` based waker (swapped file descriptors)
  (https://github.com/tokio-rs/mio/pull/1722).
* The duplicate waker check to work correctly with cloned `Registry`s.
  (https://github.com/tokio-rs/mio/pull/1706).

# 0.8.8

## Fixed

* Fix compilation on WASI (https://github.com/tokio-rs/mio/pull/1676).

# 0.8.7

## Added

* Add/fix support for tvOS and watchOS, Mio should now build for tvOS and
  watchOS, but we don't have a CI setup yet
  (https://github.com/tokio-rs/mio/pull/1658).

## Changed

* Made the `log` dependency optional behind the `log` feature flag (enabled by
  default). Users that disabled Mio's default features will now not see any
  logging from Mio, enabling the `log` feature will fix that. This was done in
  response to the `log` crate increasing it's MSRV to v1.60, see
  https://github.com/rust-lang/log/pull/552
  (https://github.com/tokio-rs/mio/pull/1673).
* Update windows-sys dependency to v0.48
  (https://github.com/tokio-rs/mio/pull/1663).

## Fixed

* Fix overflow in `Poll::poll` when using `Duration::MAX` as timeout
  (https://github.com/tokio-rs/mio/pull/1657).

# 0.8.6

## Added

* `Interest::PRIORITY` on Linux and Android, to trigger `Event::is_priority`
  (https://github.com/tokio-rs/mio/pull/1647).

## Changed

* Updated windows-sys to 0.45
  (https://github.com/tokio-rs/mio/pull/1644).
* We started testing with sanitizers on the CI
  (https://github.com/tokio-rs/mio/pull/1648).

## Fixed

* A number of potential fd leaks when setup resulted in an error right after
  creation (https://github.com/tokio-rs/mio/pull/1636).
* Less truncating for timeout values in `Poll::poll`
  (https://github.com/tokio-rs/mio/pull/1642).

# 0.8.5

## Changed

* Updated `windows-sys` to 0.42.0
  (https://github.com/tokio-rs/mio/pull/1624).
* Officially document Wine as not supported, some people claimed it worked,
  other claims it doesn't, but nobody stepped up to fix the problem
  (https://github.com/tokio-rs/mio/pull/1596).
* Switch to GitHub Actions
  (https://github.com/tokio-rs/mio/pull/1598, https://github.com/tokio-rs/mio/pull/1601).
* Documented the current Poll::poll time behaviour
  (https://github.com/tokio-rs/mio/pull/1603).

## Fixed

* Timeout less than one millisecond becoming zero millsiconds
  (https://github.com/tokio-rs/mio/pull/1615, https://github.com/tokio-rs/mio/pull/1616)
* Undefined reference to `epoll\_create1` on Android API level < 21.
  (https://github.com/tokio-rs/mio/pull/1590).

# 0.8.4

## Added

* Support `Registery::try_clone` on `wasm32-wasi`
  (https://github.com/tokio-rs/mio/pull/1576).
* Add docs about polling without registering event sources
  (https://github.com/tokio-rs/mio/pull/1585).

# 0.8.3

## Changed

* Replace `winapi` dependency with `windows-sys`.
  (https://github.com/tokio-rs/mio/pull/1556).
* Future proofed the kevent ABI for FreeBSD
  (https://github.com/tokio-rs/mio/pull/1572).

## Fixed

* Improved support for Redox, making it possible to run on stable Rust
  (https://github.com/tokio-rs/mio/pull/1555).
* Don't ignore EAGAIN in UDS connect call
  (https://github.com/tokio-rs/mio/pull/1564).
* Documentation of `TcpStream::connect`
  (https://github.com/tokio-rs/mio/pull/1565).

# 0.8.2

## Added

* Experimental support for Redox.

# 0.8.1

## Added

* Add `try_io` method to all I/O types (#1551). This execute a user defined I/O
  closure while updating Mio's internal state ensuring that the I/O type
  receives more events if it hits a WouldBlock error. This is added to the
  following types:
   * `TcpStream`
   * `UdpSocket`
   * `UnixDatagram`
   * `UnixStream`
   * `unix::pipe::Sender`
   * `unix::pipe::Receiver`
* Basic, experimental support for `wasm32-wasi` target (#1549). Note that a lot
  of time type are still missing, e.g. the `Waker`, and may never be possible to
  implement.

# 0.8.0

## Removed

* Deprecated features (https://github.com/tokio-rs/mio/commit/105f8f2afb57b01ddea716a0aa9720f226c520e3):
  * extra-docs (always enabled)
  * tcp (replaced with "net" feature).
  * udp (replaced with "net" feature).
  * uds (replaced with "net" feature).
  * pipe (replaced with "os-ext" feature).
  * os-util (replaced with "os-ext" feature).
* `TcpSocket` type
  (https://github.com/tokio-rs/mio/commit/02e9be41f27daf822575444fdd2b3067433a5996).
  The socket2 crate provides all the functionality and more.
* Support for Solaris, it never really worked anyway
  (https://github.com/tokio-rs/mio/pull/1528).

## Changes

* Update minimum Rustc version (MSVR) to 1.46.0
  (https://github.com/tokio-rs/mio/commit/5c577efecd23750a9a3e0f6ad080ab98f14a255d).

## Added

* `UdpSocket::peer_addr`
  (https://github.com/tokio-rs/mio/commit/5fc104d08e0e74c8a19247f7cba0f058699fc438).

# 0.7.14

## Fixes

* Remove use unsound internal macro (#1519).

## Added

* `sys::unix::SocketAddr::as_abstract_namespace()` (#1520).

# 0.7.13

## Fixes

* Fix `Registry::try_clone` invalid usage of `F_DUPFD_CLOEXEC` (#1497,
  https://github.com/tokio-rs/mio/commit/2883f5c1f35bf1a59682c5ffc4afe6b97d7d6e68).

# 0.7.12 (yanked)

## Fixes

* Set `FD_CLOEXEC` when calling `Registry::try_clone`
  (https://github.com/tokio-rs/mio/commit/d1617b567ff6bc669d71e367d22e0e93ff7e2e24 for epoll and
  (https://github.com/tokio-rs/mio/commit/b367a05e408ca90a26383c3aa16d8a16f019dc59 for kqueue).

# 0.7.11

## Fixes

* Fix missing feature of winapi.
  (https://github.com/tokio-rs/mio/commit/a7e61db9e3c2b929ef1a33532bfcc22045d163ce).

# 0.7.10

## Fixes

* Fix an instance of not doc(cfg(.*))
  (https://github.com/tokio-rs/mio/commit/25e8f911357c740034f10a170dfa4ea1b28234ce).

# 0.7.9

## Fixes

* Fix error handling in `NamedPipe::write`
  (https://github.com/tokio-rs/mio/commit/aec872be9732e5c6685100674278be27f54a271b).
* Use `accept(2)` on x86 Android instead of `accept4(2)`
  (https://github.com/tokio-rs/mio/commit/6f86b925d3e48f30905d5cfa54348acf3f1fa036,
  https://github.com/tokio-rs/mio/commit/8d5414880ab82178305ac1d2c16d715e58633d3e).
* Improve error message when opening AFD device
  (https://github.com/tokio-rs/mio/commit/139f7c4422321eb4a17b14ae2c296fddd19a8804).

# 0.7.8

## Fixes

* Fix `TcpStream::set_linger` on macOS
  (https://github.com/tokio-rs/mio/commit/175773ce02e85977db81224c782c8d140aba8543).
* Fix compilation on DragonFlyBSD
  (https://github.com/tokio-rs/mio/commit/b51af46b28871f8dd3233b490ee62237ffed6a26).

# 0.7.7

## Added

* `UdpSocket::only_v6`
  (https://github.com/tokio-rs/mio/commit/0101e05a800f17fb88f4315d9b9fe0f08cca6e57).
* `Clone` implementation for `Event`
  (https://github.com/tokio-rs/mio/commit/26540ebbae89df6d4d08465c56f715d8f2addfc3).
* `AsRawFd` implementation for `Registry`
  (https://github.com/tokio-rs/mio/commit/f70daa72da0042b1880256164774c3286d315a02).
* `Read` and `Write` implementation for `&unix::pipe::Sender` and `Receiver`,
  that is on the reference to them, an implementation existed on the types
  themselves already
  (https://github.com/tokio-rs/mio/commit/1be481dcbbcb6906364008b5d61e7f53cddc3eb3).

## Fixes

* Underflow in `SocketAddr::address`
  (https://github.com/tokio-rs/mio/commit/6d3fa69240cd4bb95e9d34605c660c30245a18bd).
* Android build with the net feature enabled, but with os-poll disabled
  (https://github.com/tokio-rs/mio/commit/49d8fd33e026ad6e2c055d05d6667180ba2af7be).
* Solaris build with the net feature enabled, but with os-poll disabled
  (https://github.com/tokio-rs/mio/commit/a6e025e9d9511639ec106ebedc0dd312bdc9be12).
* Ensure that `Waker::wake` works on illumos systems with poor `pipe(2)` and
  `epoll(2)` interaction using `EPOLLET`
  (https://github.com/tokio-rs/mio/commit/943d4249dcc17cd8b4d2250c4fa19116097248fa).
* Fix `unix::pipe` on illumos
  (https://github.com/tokio-rs/mio/commit/0db49f6d5caf54b12176821363d154384357e70a).

# 0.7.6

## Added

* `net` feature, replaces `tcp`, `udp` and `uds` features
  (https://github.com/tokio-rs/mio/commit/a301ba520a8479b459c4acdcefa4a7c5eea818c7).
* `os-ext` feature, replaces `os-util` and `pipe` features
  (https://github.com/tokio-rs/mio/commit/f5017fae8a3d3bb4b4cada25b01a2d76a406badc).
* Added keepalive support to `TcpSocket`
  (https://github.com/tokio-rs/mio/commit/290c43a96662d54ab7c4b8814e5a9f9a9e523fda).
* `TcpSocket::set_{send, recv}_buffer_size`
  (https://github.com/tokio-rs/mio/commit/40c4af79bf5b32b8fbdbf6f2e5c16290e1d3d406).
* `TcpSocket::get_linger`
  (https://github.com/tokio-rs/mio/commit/13e82ced655bbb6e2729226e485a7de9f2c2ccd9).
* Implement `IntoRawFd` for `TcpSocket`
  (https://github.com/tokio-rs/mio/commit/50548ed45d0b2c98f1f2e003e210d14195284ef4).

## Deprecated

* The `tcp`, `udp` and `uds` features, replaced by a new `net` feature.
  (https://github.com/tokio-rs/mio/commit/a301ba520a8479b459c4acdcefa4a7c5eea818c7).
* The `extra-docs` feature, now enabled by default.
  (https://github.com/tokio-rs/mio/commit/25731e8688a2d91c5c700674a2c2d3841240ece1).
* The `os-util` and `pipe` features, replaced by a new `os-ext` feature.
  (https://github.com/tokio-rs/mio/commit/f5017fae8a3d3bb4b4cada25b01a2d76a406badc).

## Fixes

* Incorrect assumption of the layout of `std::net::SocketAddr`. Previously Mio
  would assume that `SocketAddrV{4,6}` had the same layout as
  `libc::sockaddr_in(6)`, however this is not guaranteed by the standard
  library.
  (https://github.com/tokio-rs/mio/commit/152e0751f0be1c9b0cbd6778645b76bcb0eba93c).
* Also bumped the miow dependency to version 0.3.6 to solve the same problem as
  above.

# 0.7.5

## Added

* `TcpSocket::get_localaddr()` retrieves local address
  (https://github.com/tokio-rs/mio/commit/b41a022b2242eef1969c70c8ba93e04c528dba47).
* `TcpSocket::set_reuseport()` & `TcpSocket::get_reuseport()` configures and reads `SO_REUSEPORT`
  (https://github.com/tokio-rs/mio/commit/183bbe409ab69cbf9db41d0263b41ec86202d9a0).
* `unix:pipe()` a wrapper around pipe(2) sys call
  (https://github.com/tokio-rs/mio/commit/2b7c0967a7362303946deb3d4ca2ae507af6c72d).
* Add a check that a single Waker is active per Poll instance (only in debug mode)
  (https://github.com/tokio-rs/mio/commit/f4874f28b32efcf4841691884c65a89734d96a56).
* Added `Interest:remove()`
  (https://github.com/tokio-rs/mio/commit/b8639c3d9ac07bb7e2e27685680c8a6510fa1357).

# 0.7.4

## Fixes

* lost "socket closed" events on windows
  (https://github.com/tokio-rs/mio/commit/50c299aca56c4a26e5ed20c283007239fbe6a7a7).

## Added

* `TcpSocket::set_linger()` configures SO_LINGER
  (https://github.com/tokio-rs/mio/commit/3b4096565c1a879f651b8f8282ecdcbdbd5c92d3).

# 0.7.3

## Added

* `TcpSocket` for configuring a TCP socket before connecting or listening
  (https://github.com/tokio-rs/mio/commit/5b09e60d0f64419b989bda88c86a3147334a03b3).

# 0.7.2

## Added

* Windows named pipe support.
  (https://github.com/tokio-rs/mio/commit/52e8c2220e87696d20f13561402bcaabba4136ed).

# 0.7.1

## Reduced support for 32-bit Apple targets

In January 2020 Rust reduced its support for 32-bit Apple targets
(https://blog.rust-lang.org/2020/01/03/reducing-support-for-32-bit-apple-targets.html).
Starting with v0.7.1 Mio will do the same as we're no longer checking 32 bit
iOS/macOS on our CI.

## Added

* Support for illumos
  (https://github.com/tokio-rs/mio/commit/976f2354d0e8fbbb64fba3bf017d7131f9c369a0).
* Report `epoll(2)`'s `EPOLLERR` event as `Event::is_write_closed` if it's the
  only event
  (https://github.com/tokio-rs/mio/commit/0c77b5712d675eeb9bd43928b5dd7d22b2c7ac0c).
* Optimised event::Iter::{size_hint, count}
  (https://github.com/tokio-rs/mio/commit/40df934a11b05233a7796c4de19a4ee06bc4e03e).

## Fixed

* Work around Linux kernel < 2.6.37 bug on 32-bits making timeouts longer then
  ~30 minutes effectively infinite
  (https://github.com/tokio-rs/mio/commit/d555991f5ee81f6c1eec0fe481557d3d5b8d5ff4).
* Set `SO_NOSIGPIPE` on all sockets (not just UDP) on for Apple targets
  (https://github.com/tokio-rs/mio/commit/b8bbdcb0d3236f4c4acb257996d42a88dc9987d9).
* Properly handle `POLL_ABORT` on Windows
  (https://github.com/tokio-rs/mio/commit/a98da62b3ed1eeed1770aaca12f46d647e4fa749).
* Improved error handling around failing `SIO_BASE_HANDLE` calls on Windows
  (https://github.com/tokio-rs/mio/commit/b15fc18458a79ef8a51f73effa92548650f4e5dc).

## Changed

* On NetBSD we now use `accept4(2)`
  (https://github.com/tokio-rs/mio/commit/4e306addc7144f2e02a7e8397c220b179a006a19).
* The package uploaded to crates.io should be slightly smaller
  (https://github.com/tokio-rs/mio/commit/eef8d3b9500bc0db957cd1ac68ee128ebc68351f).

## Removed

* Dependency on `lazy_static` on Windows
  (https://github.com/tokio-rs/mio/commit/57e4c2a8ac153bc7bb87829e22cf0a21e3927e8a).

# 0.7.0

Version 0.7 of Mio contains various major changes compared to version 0.6.
Overall a large number of API changes have been made to reduce the complexity of
the implementation and remove overhead where possible.

Please refer to the [blog post about
0.7-alpha.1](https://tokio.rs/blog/2019-12-mio-v0.7-alpha.1/) for additional
information.

## Added

* `Interest` structure that replaces `Ready` in registering event sources.
* `Registry` structure that separates the registering and polling functionality.
* `Waker` structure that allows another thread to wake a thread polling `Poll`.
* Unix Domain Socket (UDS) types: `UnixDatagram`, `UnixListener` and
  `UnixStream`.

## Removed

* All code deprecated in 0.6 was removed in 0.7.
* Support for Fuchsia was removed as the code was unmaintained.
* Support for Bitrig was removed, rustc dropped support for it also.
* `UnixReady` was merged into `Ready`.
* Custom user-space readiness queue was removed, this includes the public
  `Registration` and `SetReadiness` types.
* `PollOpt` was removed and all registrations use edge-triggers. See the upgrade
  guide on how to process event using edge-triggers.
* The network types (types in the `net` module) now support only the same API as
  found in the standard library, various methods on the types were removed.
* `TcpStream` now supports vectored I/O.
* `Poll::poll_interruptible` was removed. Instead `Poll::poll` will now return
  an error if one occurs.
* `From<usize>` is removed from `Token`, the internal field is still public, so
  `Token(my_token)` can still be used.

## Changed

* Various documentation improvements were made around correct usage of `Poll`
  and registered event sources. It is recommended to reread the documentation of
  at least `event::Source` and `Poll`.
* Mio now uses Rust 2018 and rustfmt for all code.
* `Event` was changed to be a wrapper around the OS event. This means it can be
  significantly larger on some OSs.
* `Ready` was removed and replaced with various `is_*` methods on `Event`. For
  example instead checking for readable readiness using
  `Event::ready().is_readable()`, you would call `Event::is_readable()`.
* `Ready::is_hup` was removed in favour of `Event::is_read_closed` and
  `Event::is_write_closed`.
* The Iterator implementation of `Events` was changed to return `&Event`.
* `Evented` was renamed to `event::Source` and now takes mutable reference to
  the source.
* Minimum supported Rust version was increased to 1.39.
* By default Mio now uses a shim implementation. To enable the full
  implementation, that uses the OS, enable the `os-oll` feature. To enable the
  network types use `tcp`, `udp` and/or `uds`. For more documentation on the
  features see the `feature` module in the API documentation (requires the
  `extra-docs` feature).
* The entire Windows implementation was rewritten.
* Various optimisation were made to reduce the number of system calls in
  creating and using sockets, e.g. making use of `accept4(2)`.
* The `fmt::Debug` implementation of `Events` is now actually useful as it
  prints all `Event`s.

# 0.6.23 (Dec 01, 2020)

### Changed
- **MSRV**: Increased the MSRV from 1.18.0 (Jun 8, 2017) to 1.31.0 (Dec 6,
  2018)
  (https://github.com/tokio-rs/mio/commit/4879e0d32ddfd98e762fc87240e594a3ad8fca30).

### Fixed
- Work around Linux kernel < 2.6.37 bug on 32-bits making timeouts longer then
  ~30 minutes effectively infinite
  (https://github.com/tokio-rs/mio/commit/e7cba59950e9c9fa6194e29b5b1e72029e3df455).
- Update miow and net2 dependencies to get rid of invalid memory layout assumption
  (https://github.com/tokio-rs/mio/commit/13f02ac0a86d7c0c0001e5ff8960a0b4340d075c).

# 0.6.22 (May 01, 2020)

### Added
- Add support for illumos target (#1294)

# 0.6.21 (November 27, 2019)

### Fixed
- remove `=` dependency on `cfg-if`.

# 0.6.20 (November 21, 2019)

### Fixed
- Use default IOCP concurrency value (#1161).
- setting FD_CLOEXEC in pipe (#1095).

# 0.6.19 (May 28, 2018)

### Fixed
- Do not trigger HUP events on kqueue platforms (#958).

# 0.6.18 (May 24, 2018)

### Fixed
- Fix compilation on kqueue platforms with 32bit C long (#948).

# 0.6.17 (May 15, 2018)

### Fixed
- Don't report `RDHUP` as `HUP` (#939)
- Fix lazycell related compilation issues.
- Fix EPOLLPRI conflicting with READABLE
- Abort process on ref count overflows

### Added
- Define PRI on all targets

# 0.6.16 (September 5, 2018)

* Add EPOLLPRI readiness to UnixReady on supported platforms (#867)
* Reduce spurious awaken calls (#875)

# 0.6.15 (July 3, 2018)

* Implement `Evented` for containers (#840).
* Fix android-aarch64 build (#850).

# 0.6.14 (March 8, 2018)

* Add `Poll::poll_interruptible` (#811)
* Add `Ready::all` and `usize` conversions (#825)

# 0.6.13 (February 5, 2018)

* Fix build on DragonFlyBSD.
* Add `TcpListener::from_std` that does not require the socket addr.
* Deprecate `TcpListener::from_listener` in favor of from_std.

# 0.6.12 (January 5, 2018)

* Add `TcpStream::peek` function (#773).
* Raise minimum Rust version to 1.18.0.
* `Poll`: retry select() when interrupted by a signal (#742).
* Deprecate `Events` index access (#713).
* Add `Events::clear` (#782).
* Add support for `lio_listio` (#780).

# 0.6.11 (October 25, 2017)

* Allow register to take empty interest (#640).
* Fix bug with TCP errors on windows (#725).
* Add TcpListener::accept_std (#733).
* Update IoVec to fix soundness bug -- includes behavior change. (#747).
* Minimum Rust version is now 1.14.0.
* Fix Android x86_64 build.
* Misc API & doc polish.

# 0.6.10 (July 27, 2017)

* Experimental support for Fuchsia
* Add `only_v6` option for UDP sockets
* Fix build on NetBSD
* Minimum Rust version is now 1.13.0
* Assignment operators (e.g. `|=`) are now implemented for `Ready`

# 0.6.9 (June 7, 2017)

* More socket options are exposed through the TCP types, brought in through the
  `net2` crate.

# 0.6.8 (May 26, 2017)

* Support Fuchia
* POSIX AIO support
* Fix memory leak caused by Register::new2
* Windows: fix handling failed TCP connections
* Fix build on aarch64-linux-android
* Fix usage of `O_CLOEXEC` with `SETFL`

# 0.6.7 (April 27, 2017)

* Ignore EPIPE coming out of `kevent`
* Timer thread should exit when timer is dropped.

# 0.6.6 (March 22, 2017)

* Add send(), recv() and connect() to UDPSocket.
* Fix bug in custom readiness queue
* Move net types into `net` module

# 0.6.5 (March 14, 2017)

* Misc improvements to kqueue bindings
* Add official support for iOS, Android, BSD
* Reimplement custom readiness queue
* `Poll` is now `Sync`
* Officially deprecate non-core functionality (timers, channel, etc...)
* `Registration` now implements `Evented`
* Fix bug around error conditions with `connect` on windows.
* Use iovec crate for scatter / gather operations
* Only support readable and writable readiness on all platforms
* Expose additional readiness in a platform specific capacity

# 0.6.4 (January 24, 2017)

* Fix compilation on musl
* Add `TcpStream::from_stream` which converts a std TCP stream to Mio.

# 0.6.3 (January 22, 2017)

* Implement readv/writev for `TcpStream`, allowing vectored reads/writes to
  work across platforms
* Remove `nix` dependency
* Implement `Display` and `Error` for some channel error types.
* Optimize TCP on Windows through `SetFileCompletionNotificationModes`

# 0.6.2 (December 18, 2016)

* Allow registration of custom handles on Windows (like `EventedFd` on Unix)
* Send only one byte for the awakener on Unix instead of four
* Fix a bug in the timer implementation which caused an infinite loop

# 0.6.1 (October 30, 2016)

* Update dependency of `libc` to 0.2.16
* Fix channel `dec` logic
* Fix a timer bug around timeout cancellation
* Don't allocate buffers for TCP reads on Windows
* Touched up documentation in a few places
* Fix an infinite looping timer thread on OSX
* Fix compile on 32-bit OSX
* Fix compile on FreeBSD

# 0.6.0 (September 2, 2016)

* Shift primary API towards `Poll`
* `EventLoop` and types to `deprecated` mod. All contents of the
  `deprecated` mod will be removed by Mio 1.0.
* Increase minimum supported Rust version to 1.9.0
* Deprecate unix domain socket implementation in favor of using a
  version external to Mio. For example: https://github.com/alexcrichton/mio-uds.
* Remove various types now included in `std`
* Updated TCP & UDP APIs to match the versions in `std`
* Enable implementing `Evented` for any type via `Registration`
* Rename `IoEvent` -> `Event`
* Access `Event` data via functions vs. public fields.
* Expose `Events` as a public type that is passed into `Poll`
* Use `std::time::Duration` for all APIs that require a time duration.
* Polled events are now retrieved via `Events` type.
* Implement `std::error::Error` for `TimerError`
* Relax `Send` bound on notify messages.
* Remove `Clone` impl for `Timeout` (future proof)
* Remove `mio::prelude`
* Remove `mio::util`
* Remove dependency on bytes

# 0.5.0 (December 3, 2015)

* Windows support (#239)
* NetBSD support (#306)
* Android support (#295)
* Don't re-export bytes types
* Renamed `EventLoop::register_opt` to `EventLoop::register` (#257)
* `EventLoopConfig` is now a builder instead of having public struct fields. It
  is also no longer `Copy`. (#259)
* `TcpSocket` is no longer exported in the public API (#262)
* Integrate with net2. (#262)
* `TcpListener` now returns the remote peer address from `accept` as well (#275)
* The `UdpSocket::{send_to, recv_from}` methods are no longer generic over `Buf`
  or `MutBuf` but instead take slices directly. The return types have also been
  updated to return the number of bytes transferred. (#260)
* Fix bug with kqueue where an error on registration prevented the
  changelist from getting flushed (#276)
* Support sending/receiving FDs over UNIX sockets (#291)
* Mio's socket types are permanently associated with an EventLoop (#308)
* Reduce unnecessary poll wakeups (#314)


# 0.4.1 (July 21, 2015)

* [BUGFIX] Fix notify channel concurrency bug (#216)

# 0.4.0 (July 16, 2015)

* [BUGFIX] EventLoop::register requests all events, not just readable.
* [BUGFIX] Attempting to send a message to a shutdown event loop fails correctly.
* [FEATURE] Expose TCP shutdown
* [IMPROVEMENT] Coalesce readable & writable into `ready` event (#184)
* [IMPROVEMENT] Rename TryRead & TryWrite function names to avoid conflict with std.
* [IMPROVEMENT] Provide TCP and UDP types in Mio (path to windows #155)
* [IMPROVEMENT] Use clock_ticks crate instead of time (path to windows #155)
* [IMPROVEMENT] Move unix specific features into mio::unix module
* [IMPROVEMENT] TcpListener sets SO_REUSEADDR by default
