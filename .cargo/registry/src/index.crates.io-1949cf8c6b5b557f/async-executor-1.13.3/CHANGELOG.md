# Version 1.13.3

- Avoid places where the code had a possibility to block or panic. (#147)

# Version 1.13.2

- Fix build failure with minimal-versions. (#132)
- Prevent executor from becoming unusable by panic of the iterator passed by the user to the `spawn_many`. (#136)
- Reduce memory footprint. (#137)

# Version 1.13.1

- Fix docs.rs build. (#125)

# Version 1.13.0

- Relax the `Send` bound on `LocalExecutor::spawn_many`. (#120)
- Ensure all features are documented on `docs.rs`. (#122)

# Version 1.12.0

- Add static executors, which are an optimization over executors that are kept
  around forever. (#112)

# Version 1.11.0

- Re-export the `async_task::FallibleTask` primitive. (#113)
- Support racy initialization of the executor state. This should allow the executor to be
  initialized on web targets without any issues. (#108)

# Version 1.10.0

- Add a function `spawn_batch` that allows users to spawn multiple tasks while only locking the executor once. (#92)

# Version 1.9.1

- Remove the thread-local optimization due to the bugs that it introduces. (#106)

# Version 1.9.0

- Re-introduce the thread-local task push optimization to the executor. (#93)
- Bump `async-task` to v4.4.0. (#90)
- Replace some unnecessary atomic operations with non-atomic operations. (#94)
- Use weaker atomic orderings for notifications. (#95)
- When spawning a future, avoid looking up the ID to assign to that future twice. (#96)

# Version 1.8.0

- When spawned tasks panic, the panic is caught and then surfaced in the spawned
 `Task`. Previously, the panic would be surfaced in `tick()` or `run()`. (#78)

# Version 1.7.2

- Fix compilation under WebAssembly targets (#77).

# Version 1.7.1

- Fix compilation under WebAssembly targets (#75).
- Add a disclaimer indicating that this is a reference executor (#74).

# Version 1.7.0

- Bump `async-lock` and `futures-lite` to their latest versions. (#70)

# Version 1.6.0

- Remove the thread-local queue optimization, as it caused a number of bugs in production use cases. (#61)

# Version 1.5.4

- Fix a panic that could happen when two concurrent `run()` calls are made and the thread local task slot is left as `None`. (#55)

# Version 1.5.3

- Fix an accidental breaking change in v1.5.2, where `ex.run()` was no longer `Send`. (#50)
- Remove the unused `memchr` dependency. (#51)

# Version 1.5.2

- Add thread-local task queue optimizations, allowing new tasks to avoid using the global queue. (#37)
- Update `fastrand` to v2. (#45)

# Version 1.5.1

- Implement a better form of debug output for Executor and LocalExecutor. (#33)

# Version 1.5.0

- Remove the dependency on the `once_cell` crate to restore the MSRV. (#29)
- Update `concurrent-queue` to v2.

# Version 1.4.1

- Remove dependency on deprecated `vec-arena`. (#23)

# Version 1.4.0

- Add `Executor::is_empty()` and `LocalExecutor::is_empty()`.

# Version 1.3.0

- Parametrize executors over a lifetime to allow spawning non-`static` futures.

# Version 1.2.0

- Update `async-task` to v4.

# Version 1.1.1

- Replace `AtomicU64` with `AtomicUsize`.

# Version 1.1.0

- Use atomics to make `Executor::run()` and `Executor::tick()` futures `Send + Sync`.

# Version 1.0.0

- Stabilize.

# Version 0.2.1

- Add `try_tick()` and `tick()` methods.

# Version 0.2.0

- Redesign the whole API.

# Version 0.1.2

- Add the `Spawner` API.

# Version 0.1.1

- Initial version
