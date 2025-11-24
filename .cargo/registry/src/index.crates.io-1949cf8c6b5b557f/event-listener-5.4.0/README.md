# event-listener

[![Build](https://github.com/smol-rs/event-listener/workflows/CI/badge.svg)](
https://github.com/smol-rs/event-listener/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](
https://github.com/smol-rs/event-listener)
[![Cargo](https://img.shields.io/crates/v/event-listener.svg)](
https://crates.io/crates/event-listener)
[![Documentation](https://docs.rs/event-listener/badge.svg)](
https://docs.rs/event-listener)

Notify async tasks or threads.

This is a synchronization primitive similar to [eventcounts] invented by Dmitry Vyukov.

You can use this crate to turn non-blocking data structures into async or blocking data
structures. See a [simple mutex] implementation that exposes an async and a blocking interface
for acquiring locks.

[eventcounts]: https://www.1024cores.net/home/lock-free-algorithms/eventcounts
[simple mutex]: ./examples/mutex.rs

## Examples

Wait until another thread sets a boolean flag:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use event_listener::Event;

let flag = Arc::new(AtomicBool::new(false));
let event = Arc::new(Event::new());

// Spawn a thread that will set the flag after 1 second.
thread::spawn({
    let flag = flag.clone();
    let event = event.clone();
    move || {
        // Wait for a second.
        thread::sleep(Duration::from_secs(1));

        // Set the flag.
        flag.store(true, Ordering::SeqCst);

        // Notify all listeners that the flag has been set.
        event.notify(usize::MAX);
    }
});

// Wait until the flag is set.
loop {
    // Check the flag.
    if flag.load(Ordering::SeqCst) {
        break;
    }

    // Start listening for events.
    let listener = event.listen();

    // Check the flag again after creating the listener.
    if flag.load(Ordering::SeqCst) {
        break;
    }

    // Wait for a notification and continue the loop.
    listener.wait();
}
```

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
