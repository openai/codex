# Version 2.5.0

- Add `Sender::closed()` (#102)

# Version 2.4.0

- Add `Sender::same_channel()` and `Receiver::same_channel()`. (#98)
- Add `portable-atomic` feature to support platforms without atomics. (#106)

# Version 2.3.1

- Use the correct version of `async-channel` in our manifest. (#93)

# Version 2.3.0

- Add `force_send` for sending items over the channel that displace other items. (#89)

# Version 2.2.1

- Fix the CI badge in the `crates.io` page. (#84)

# Version 2.2.0

- Bump `event-listener` to v5.0.0. (#79)
- Bump MSRV to 1.60. (#80)

# Version 2.1.1

- Bump `event-listener` to v4.0.0. (#73)

# Version 2.1.0

- Bump `futures-lite` to its latest version. (#70)

# Version 2.0.0

- **Breaking:** Make `Send`, `Recv` and `Receiver` `!Unpin`. This enables more efficient event notification strategies. (#59)
- **Breaking:** Add an `std` enabled-by-default feature that enables parts of the API that require `std`. (#59)
- Add support for the `wasm32` target. (#67)

# Version 1.9.0

- Fix a bug where `WeakSender/WeakReceiver` could incorrectly return `Some` even if the channel is already closed (#60)
- Remove the unnecessary `T: Clone` bound from `WeakSender/WeakReceiver`'s `Clone` implementation (#62)

# Version 1.8.0

- Prevent deadlock if sender/receiver is forgotten (#49)
- Add weak sender and receiver (#51)
- Update `concurrent-queue` to v2 (#50)

# Version 1.7.1

- Work around MSRV increase due to a cargo bug.

# Version 1.7.0

- Add `send_blocking` and `recv_blocking` (#47)

# Version 1.6.1

- Make `send` return `Send` (#34)

# Version 1.6.0

- Added `Send` and `Recv` futures (#33)
- impl `FusedStream` for `Receiver` (#30)

# Version 1.5.1

- Fix typos in the docs.

# Version 1.5.0

- Add `receiver_count()` and `sender_count()`.

# Version 1.4.2

- Fix a bug that would sometime cause 100% CPU usage.

# Version 1.4.1

- Update dependencies.

# Version 1.4.0

- Update dependencies.

# Version 1.3.0

- Add `Sender::is_closed()` and `Receiver::is_closed()`.

# Version 1.2.0

- Add `Sender::close()` and `Receiver::close()`.

# Version 1.1.1

- Replace `usize::MAX` with `std::usize::MAX`.

# Version 1.1.0

- Add methods to error types.

# Version 1.0.0

- Initial version
