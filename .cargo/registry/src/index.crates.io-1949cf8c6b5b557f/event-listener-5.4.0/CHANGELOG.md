# Version 5.4.0

- Add a `no_std` implementation based on the `critical-section` crate, enabled
  via the feature of the same name. (#148)

# Version 5.3.1

- Disable some optimizations that, in rare conditions, can cause race conditions
  causing notifications to be dropped. (#139)
- Ensure the portable-atomic feature is set properly. (#134)
- Update `portable-atomic-util` to v0.2.0. (#132)
- Document the std feature. (#134)

# Version 5.3.0

- Add a `loom` implementation. This feature is unstable and is not semver-supported. (#126)
- Make the panic message for polling the `EventListener` after it has completed more clear. (#125)

# Version 5.2.0

- Make `StackSlot` `Sync`. (#121)

# Version 5.1.0

- Make `StackSlot` `Send`. (#119)

# Version 5.0.0

- **Breaking:** Rework the API to afford better usage. (#105)
  - The heap-based API of the v2.x line is back.
  - However, there is a stack-based API as an alternative.
- Add a way to get the total number of listeners. (#114)

# Version 4.0.3

- Relax MSRV to 1.60. (#110)

# Version 4.0.2

- Avoid spinning in `wait_deadline`. (#107)

# Version 4.0.1

- Fix a use-after-move error after an `EventListener` is assigned to listen to
  another `Event`. (#101)

# Version 4.0.0

- **Breaking:** Fix a footgun in the `EventListener` type. `EventListener::new()`
  now no longer takes an `&Event` as an argument, and `EventListener::listen()`
  takes the  `&Event` as an argument. Hopefully this should prevent `.await`ing
  on a listener without making sure it's listening first. (#94)

# Version 3.1.0

- Implement `UnwindSafe` and `RefUnwindSafe` for `EventListener`. This was unintentionally removed in version 3 (#96).

# Version 3.0.1

- Emphasize that `listen()` must be called on `EventListener` in documentation. (#90)
- Write useful output in `fmt::Debug` implementations. (#86)

# Version 3.0.0

- Use the `parking` crate instead of threading APIs (#27)
- Bump MSRV to 1.59 (#71)
- **Breaking:** Make this crate `no_std`-compatible on `default-features = false`. (#34)
- Create a new `event-listener-strategy` crate for abstracting over blocking/non-blocking operations. (#49)
- **Breaking:** Change the `EventListener` API to be `!Unpin`. (#51)
- Enable a feature for the `portable-atomic` crate. (#53)
- **Breaking:** Add a `Notification` trait which is used to enable tagged events. (#52)
- Add an `is_notified()` method to `Event`. (#48)
- **Breaking:** Make it so `notify()` returns the number of listeners notified. (#57)

# Version 2.5.3

- Fix fence on x86 and miri.

# Version 2.5.2

- Fix stacked borrows violation when `-Zmiri-tag-raw-pointers` is enabled. (#24)

# Version 2.5.1

- Replace spinlock with a mutex.

# Version 2.5.0

- Add `EventListener::discard()`.

# Version 2.4.0

- `Event::new()` is now a const fn.

# Version 2.3.3

- Fix a bug in `List::insert()` that was causing deadlocks.

# Version 2.3.2

- Optimization: use a simple spinlock and cache an `Entry` for less allocation.

# Version 2.3.1

- Optimization: don't initialize `Inner` when notifying `Event`.

# Version 2.3.0

- Implement `UnwindSafe`/`RefUnwindSafe` for `Event`/`EventListener`.

# Version 2.2.1

- Always keep the last waker in `EventListener::poll()`.

# Version 2.2.0

- Add `EventListener::same_event()`.

# Version 2.1.0

- Add `EventListener::listens_to()`.

# Version 2.0.1

- Replace `usize::MAX` with `std::usize::MAX`.

# Version 2.0.0

- Remove `Event::notify_one()` and `Event::notify_all()`.
- Add `Event::notify_relaxed()` and `Event::notify_additional_relaxed()`.
- Dropped notified `EventListener` now notifies one *or* one additional listener.

# Version 1.2.0

- Add `Event::notify_additional()`.

# Version 1.1.2

- Change a `Relaxed` load to `Acquire` load.

# Version 1.1.1

- Fix a bug in `EventListener::wait_timeout()`.

# Version 1.1.0

- Add `EventListener::notify()`.

# Version 1.0.1

- Reduce the complexity of `notify_all()` from O(n) to amortized O(1).
- Fix a bug where entries were notified in wrong order.
- Add tests.

# Version 1.0.0

- Initial version.
