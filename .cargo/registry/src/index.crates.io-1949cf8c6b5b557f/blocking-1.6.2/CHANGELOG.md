# Version 1.6.2

- Fix build failure with minimal-versions. (#71)

# Version 1.6.1

- Remove our dependency on the `async-lock` crate. (#59)

# Version 1.6.0

- Panics that occur in `unblock`ed functions are now propagated to the calling
  function. (#58)
- Add a new optional `tracing` feature. When enabled, this feature adds logging
  to the implementation. By default it is disabled. (#60)
- Remove the unused `fastrand` dependency. (#61)

# Version 1.5.1

- Fix compilation on WebAssembly targets (#54).

# Version 1.5.0

- Bump MSRV to 1.61. (#50)

# Version 1.4.1

- Change the `error_span` in `grow_pool` into `trace_span`. (#45)

# Version 1.4.0

- Bump MSRV to 1.59. (#44)
- Remove the unused `memchr` dependency. (#38)
- Extract read/write pipes into the `piper` crate, which this crate now uses. (#37)
- Mark as `forbid(unsafe_code)` (#37).
- Set up logging using `tracing`. (#40)

# Version 1.3.1

- Gracefully handle the inability to spawn threads. (#31)

# Version 1.3.0

- Remove the dependency on the `once_cell` crate to restore the MSRV. (#30)

# Version 1.2.0

- Return `Task` from `unblock` instead of returning opaque type. (#25)

# Version 1.1.0

- Add an environment variable to customize the maximum number of threads. (#21)

# Version 1.0.2

- Update `futures-lite`.

# Version 1.0.1

- Use `async-task`.

# Version 1.0.0

- Stabilize.

# Version 0.6.1

- Add probabilistic yielding to improve fairness.

# Version 0.6.0

- Remove the `unblock!` macro.

# Version 0.5.2

- Implement `Sync` for `Unblock`.

# Version 0.5.1

- Add `Unblock::with_capacity()`.
- Add `unblock()` function.
- An optimization in task spawning.

# Version 0.5.0

- Simplify the API to just `unblock!` and `Unblock`.

# Version 0.4.7

- Simplify dependencies for faster compilation.

# Version 0.4.6

- Update doc comment on `Unblock`.

# Version 0.4.5

- Implement `AsyncSeek`/`Seek` for `Unblock`/`BlockOn`.

# Version 0.4.4

- Remove the initial poll in block_on that caused lost wakeups.

# Version 0.4.3

- Fix a bug where a closed `Receiver` causes panics.

# Version 0.4.2

- Start thread numbering from 1.

# Version 0.4.1

- Attach names to spawned threads.

# Version 0.4.0

- Remove `Future` impl for `Blocking`.
- Add `unblock()`.
- Rename `blocking!` to `unblock!`.
- Rename `Blocking` to `Unblock`.
- Add `block_on()`, `block_on!`, and `BlockOn`.

# Version 0.3.2

- Make `Blocking` implement `Send` in more cases.

# Version 0.3.1

- Add `Blocking::with_mut()`.

# Version 0.3.0

- Remove `Blocking::spawn()`.
- Implement `Future` for `Blocking` only when the inner type is a `FnOnce`.

# Version 0.2.0

- Initial version

# Version 0.1.0

- Reserved crate name
