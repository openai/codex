# Version 0.7.2

- Add `Sender::broadcast_blocking` and `Receiver::recv_blocking`. #41
- Use `Mutex` instead of `RwLock` for securing the inner data. #42
- Many non-user-facing internal improvements and fixes.

# Version 0.7.1

- Add a `poll_recv()` method to the `Receiver` type. This allows for `Receiver`
  to be used in `poll`-based contexts. (#56)

# Version 0.7.0

- **Breaking:** `Recv` and `Send` are now `!Unpin` to allow for future optimizations.
- Port to event-listener v5.0.

# Version 0.6.0

- Bump to event-listener v3.0.0.
- Add smol-rs logo to docs.

# Version 0.5.1

-  Drop `parking_lot` dependency, in favor of sync primitives in std.

# Version 0.5.0

- API to disable waiting for active receivers (#35).

# Version 0.4.1

- Drop unneeded easy-parallel dep.
- Bumb dependencies to the current versions.
- Update `parking_lot` to 0.12.1.
- fix incorrect documentation for `TrySendError::is_disconnected`.

# Version 0.4.0

- Add `RecvError::Overflowed` for detecting missing messages.
- Avoid overflows on 32- and 16-bit systems (#22).
- Add overflow message count.
- `Clone` impl of `Receiver` now properly duplicates it.
- Add `Receiver::new_receiver`.
- Add `Receiver::new_sender` and `Sender::new_receiver`, allowing generating senders from receivers
  and vice versa, respectively.
- Switch to `parking_lot::RwLock` instead of `std::sync::Mutex`.

# Version 0.3.4

- Avoid the last clone in `try_recv` (#18).
- Add some basic benchmarks.

# Version 0.3.3

- Close channel if the last receiver to drop is inactive.

# Version 0.3.2

- Fix a underflow panic (#14).
- Document difference with other broadcast APIs.

# Version 0.3.1

- Channel API in InactiveReceiver (#11).
- {Sender,Receiver}::inactive_receiver_count method.

# Version 0.3.0

- overflow mode.
- ability to modify channel capacity.
- Inactive receivers (#2).
- Document difference to `async-channel` crate (#6).

# Version 0.2.0

- First real release.

# Version 0.1.0

- Dummy release to get the name registered on crates.io.
