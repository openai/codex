# async-task

[![Build](https://github.com/smol-rs/async-task/workflows/Build%20and%20test/badge.svg)](
https://github.com/smol-rs/async-task/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](
https://github.com/smol-rs/async-task)
[![Cargo](https://img.shields.io/crates/v/async-task.svg)](
https://crates.io/crates/async-task)
[![Documentation](https://docs.rs/async-task/badge.svg)](
https://docs.rs/async-task)

Task abstraction for building executors.

To spawn a future onto an executor, we first need to allocate it on the heap and keep some
state attached to it. The state indicates whether the future is ready for polling, waiting to
be woken up, or completed. Such a stateful future is called a *task*.

All executors have a queue that holds scheduled tasks:

```rust
let (sender, receiver) = flume::unbounded();
```

A task is created using either `spawn()`, `spawn_local()`, or `spawn_unchecked()` which
return a `Runnable` and a `Task`:

```rust
// A future that will be spawned.
let future = async { 1 + 2 };

// A function that schedules the task when it gets woken up.
let schedule = move |runnable| sender.send(runnable).unwrap();

// Construct a task.
let (runnable, task) = async_task::spawn(future, schedule);

// Push the task into the queue by invoking its schedule function.
runnable.schedule();
```

The `Runnable` is used to poll the task's future, and the `Task` is used to await its
output.

Finally, we need a loop that takes scheduled tasks from the queue and runs them:

```rust
for runnable in receiver {
    runnable.run();
}
```

Method `run()` polls the task's future once. Then, the `Runnable`
vanishes and only reappears when its `Waker` wakes the task, thus
scheduling it to be run again.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
