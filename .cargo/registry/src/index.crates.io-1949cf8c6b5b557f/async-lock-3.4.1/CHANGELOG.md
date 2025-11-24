# Version 3.4.1

- Fix typos in docs. (#89)

# Version 3.4.0

- Port to `event-listener` v5.0.0. (#74)

# Version 3.3.0

- Add a `forget()` method for semaphore guards. (#73)
- Increase MSRV to v1.60. (#75)

# Version 3.2.0

- Add missing methods for blocking on locking with types wrapped in `Arc` (#71).

# Version 3.1.2

- Bump `event-listener` to version v4.0.0. (#69)

# Version 3.1.1

- Add a note to the documentation comparing this crate against `libstd`'s locks. (#58)

# Version 3.1.0

- Add a `Default` implementation for `OnceCell` (#63).

# Version 3.0.0

- **Breaking:** Add an enabled-by-default `std` feature that allows using this crate without the standard library. (#43)
- Support blocking and non-blocking operations on the same locks. (#56)
- Switch to a more efficient event notification mechanism. (#43)

# Version 2.8.0

- Fix a bug where the `SemaphoreGuard::acquire_arc` future would busy wait under certain conditions (#42).
- Add a `Semaphore::add_permits()` function to increase the number of available permits on the semaphore (#44).
- Make `RwLockReadGuard` covariant over its lifetime (#45)
- Add `RwLockReadGuardArc`, `RwLockWriteGuardArc`, and other reference counted guards for the `RwLock` type (#47).
- Loosen the `Send`/`Sync` bounds on certain future types (#48).
- Fix UB caused by the `MutexGuardArc::source` function allowing the user to drop an object in a different thread than the one it was acquired in (#50). This is a breaking change, but in the name of soundness. Therefore it doesn't break any valid behavior.
- Fix a bug where this crate would not compile properly on `wasm64` (#51).

# Version 2.7.0

- Replace some `async` blocks with manual futures (#34)
- Remove our dependency on `futures-lite` (#36)
- Mark guard types with `#[clippy::has_significant_drop]` (#37)

# Version 2.6.0

- Add `OnceCell`. (#27)
- Support wasm64.

# Version 2.5.0

- Fix an issue where the future returned by `Mutex::lock_arc`/`Semaphore::acquire_arc` holds a reference to `self`. (#20, #21)

# Version 2.4.0

- Add WASM support. (#14)

# Version 2.3.0

- Merge all subcrates.

# Version 2.2.0

- Add functions to upgrade and downgrade `RwLock` guards.
- Make all constructors `const fn`.

# Version 2.1.3

- Add `#![forbid(unsafe_code)]`.

# Version 2.1.2

- Update dependencies.

# Version 2.1.1

- Update crate description.

# Version 2.1.0

- Add `Barrier` and `Semaphore`.

# Version 2.0.1

- Update crate description.

# Version 2.0.0

- Only re-export `async-mutex` and `async-rwlock`.

# Version 1.1.5

- Replace the implementation with `async-mutex`.

# Version 1.1.4

- Replace `usize::MAX` with `std::usize::MAX`.

# Version 1.1.3

- Update dependencies.

# Version 1.1.2

- Fix a deadlock issue.

# Version 1.1.1

- Fix some typos.

# Version 1.1.0

- Make locking fair.
- Add `LockGuard::source()`.

# Version 1.0.2

- Bump the `event-listener` version.
- Add tests.

# Version 1.0.1

- Update Cargo categories.

# Version 1.0.0

- Initial version
