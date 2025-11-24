# Want

- [Crates.io](https://crates.io/crates/want)
- [Docs](https://docs.rs/want)

A `Future`s channel-like utility to signal when a value is wanted.

Futures are supposed to be lazy, and only starting work if `Future::poll`
is called. The same is true of `Stream`s, but when using a channel as
a `Stream`, it can be hard to know if the receiver is ready for the next
value.

Put another way, given a `(tx, rx)` from `futures::sync::mpsc::channel()`,
how can the sender (`tx`) know when the receiver (`rx`) actually wants more
work to be produced? Just because there is room in the channel buffer
doesn't mean the work would be used by the receiver.

This is where something like `want` comes in. Added to a channel, you can
make sure that the `tx` only creates the message and sends it when the `rx`
has `poll()` for it, and the buffer was empty.

## License

`want` is provided under the MIT license. See [LICENSE](LICENSE).
