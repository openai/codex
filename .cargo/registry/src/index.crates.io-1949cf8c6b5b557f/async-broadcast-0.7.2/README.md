# async-broadcast

[![Build](https://github.com/smol-rs/async-broadcast/workflows/Build%20and%20test/badge.svg)](
https://github.com/smol-rs/async-broadcast/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](
https://github.com/smol-rs/async-broadcast)
[![Cargo](https://img.shields.io/crates/v/async-broadcast.svg)](
https://crates.io/crates/async-broadcast)
[![Documentation](https://docs.rs/async-broadcast/badge.svg)](
https://docs.rs/async-broadcast)

An async multi-producer multi-consumer broadcast channel, where each consumer gets a clone of every
message sent on the channel. For obvious reasons, the channel can only be used to broadcast types
that implement `Clone`.

A channel has the `Sender` and `Receiver` side. Both sides are cloneable and can be shared
among multiple threads.

When all `Sender`s or all `Receiver`s are dropped, the channel becomes closed. When a channel is
closed, no more messages can be sent, but remaining messages can still be received.

The channel can also be closed manually by calling `Sender::close()` or
`Receiver::close()`.

## Examples

```rust
use async_broadcast::{broadcast, TryRecvError};
use futures_lite::{future::block_on, stream::StreamExt};

block_on(async move {
    let (s1, mut r1) = broadcast(2);
    let s2 = s1.clone();
    let mut r2 = r1.clone();

    // Send 2 messages from two different senders.
    s1.broadcast(7).await.unwrap();
    s2.broadcast(8).await.unwrap();

    // Channel is now at capacity so sending more messages will result in an error.
    assert!(s2.try_broadcast(9).unwrap_err().is_full());
    assert!(s1.try_broadcast(10).unwrap_err().is_full());

    // We can use `recv` method of the `Stream` implementation to receive messages.
    assert_eq!(r1.next().await.unwrap(), 7);
    assert_eq!(r1.recv().await.unwrap(), 8);
    assert_eq!(r2.next().await.unwrap(), 7);
    assert_eq!(r2.recv().await.unwrap(), 8);

    // All receiver got all messages so channel is now empty.
    assert_eq!(r1.try_recv(), Err(TryRecvError::Empty));
    assert_eq!(r2.try_recv(), Err(TryRecvError::Empty));

    // Drop both senders, which closes the channel.
    drop(s1);
    drop(s2);

    assert_eq!(r1.try_recv(), Err(TryRecvError::Closed));
    assert_eq!(r2.try_recv(), Err(TryRecvError::Closed));
})
```

## Difference with `async-channel`

This crate is similar to [`async-channel`] in that they both provide an MPMC channel but the main
difference being that in `async-channel`, each message sent on the channel is only received by one
of the receivers. `async-broadcast` on the other hand, delivers each message to every receiver
(IOW broadcast) by cloning it for each receiver.

[`async-channel`]: https://crates.io/crates/async-channel

## Difference with other broadcast crates

* [`broadcaster`]: The main difference would be that `broadcaster` doesn't have a sender and
  receiver split and both sides use clones of the same BroadcastChannel instance. The messages are
  sent are sent to all channel clones. While this can work for many cases, the lack of sender and
  receiver split, means that often times, you'll find yourself having to drain the channel on the
  sending side yourself.

* [`postage`]: this crate provides a [broadcast API][pba] similar to `async_broadcast`. However, it:
  - (at the time of this writing) duplicates [futures] API, which isn't ideal.
  - Does not support overflow mode nor has the concept of inactive receivers, so a slow or inactive
    receiver blocking the whole channel is not a solvable problem.
  - Provides all kinds of channels, which is generally good but if you just need a broadcast
    channel, `async_broadcast` is probably a better choice.

* [`tokio::sync`]: Tokio's `sync` module provides a [broadcast channel][tbc] API. The differences
   here are:
  - While this implementation does provide [overflow mode][tom], it is the default behavior and not
    opt-in.
  - There is no equivalent of inactive receivers.
  - While it's possible to build tokio with only the `sync` module, it comes with other APIs that
    you may not need.

[`broadcaster`]: https://crates.io/crates/broadcaster
[`postage`]: https://crates.io/crates/postage
[pba]: https://docs.rs/postage/0.4.1/postage/broadcast/fn.channel.html
[futures]: https://crates.io/crates/futures
[`tokio::sync`]: https://docs.rs/tokio/1.6.0/tokio/sync
[tbc]: https://docs.rs/tokio/1.6.0/tokio/sync/broadcast/index.html
[tom]: https://docs.rs/tokio/1.6.0/tokio/sync/broadcast/index.html#lagging

## Safety
This crate uses ``#![deny(unsafe_code)]`` to ensure everything is implemented in
100% Safe Rust.

## Contributing
Want to join us? Check out our ["Contributing" guide][contributing] and take a
look at some of these issues:

- [Issues labeled "good first issue"][good-first-issue]
- [Issues labeled "help wanted"][help-wanted]

[contributing]: https://github.com/smol-rs/async-broadcast/blob/master/.github/CONTRIBUTING.md
[good-first-issue]: https://github.com/smol-rs/async-broadcast/labels/good%20first%20issue
[help-wanted]: https://github.com/smol-rs/async-broadcast/labels/help%20wanted

## License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br/>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
