# async-io

[![Build](https://github.com/smol-rs/async-io/actions/workflows/ci.yml/badge.svg)](
https://github.com/smol-rs/async-io/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](
https://github.com/smol-rs/async-io)
[![Cargo](https://img.shields.io/crates/v/async-io.svg)](
https://crates.io/crates/async-io)
[![Documentation](https://docs.rs/async-io/badge.svg)](
https://docs.rs/async-io)

Async I/O and timers.

This crate provides two tools:

* `Async`, an adapter for standard networking types (and [many other] types) to use in
  async programs.
* `Timer`, a future that expires at a point in time.

For concrete async networking types built on top of this crate, see [`async-net`].

[many other]: https://github.com/smol-rs/async-io/tree/master/examples
[`async-net`]: https://docs.rs/async-net

## Implementation

The first time `Async` or `Timer` is used, a thread named "async-io" will be spawned.
The purpose of this thread is to wait for I/O events reported by the operating system, and then
wake appropriate futures blocked on I/O or timers when they can be resumed.

To wait for the next I/O event, the "async-io" thread uses [epoll] on Linux/Android/illumos,
[kqueue] on macOS/iOS/BSD, [event ports] on illumos/Solaris, and [IOCP] on Windows. That
functionality is provided by the [`polling`] crate.

However, note that you can also process I/O events and wake futures on any thread using the
`block_on()` function. The "async-io" thread is therefore just a fallback mechanism
processing I/O events in case no other threads are.

[epoll]: https://en.wikipedia.org/wiki/Epoll
[kqueue]: https://en.wikipedia.org/wiki/Kqueue
[event ports]: https://illumos.org/man/port_create
[IOCP]: https://learn.microsoft.com/en-us/windows/win32/fileio/i-o-completion-ports
[`polling`]: https://docs.rs/polling

## Examples

Connect to `example.com:80`, or time out after 10 seconds.

```rust
use async_io::{Async, Timer};
use futures_lite::{future::FutureExt, io};

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

let addr = "example.com:80".to_socket_addrs()?.next().unwrap();

let stream = Async::<TcpStream>::connect(addr).or(async {
    Timer::after(Duration::from_secs(10)).await;
    Err(io::ErrorKind::TimedOut.into())
})
.await?;
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
