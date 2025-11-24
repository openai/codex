# parking

[![Build](https://github.com/smol-rs/parking/workflows/Build%20and%20test/badge.svg)](
https://github.com/smol-rs/parking/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](
https://github.com/smol-rs/parking)
[![Cargo](https://img.shields.io/crates/v/parking.svg)](
https://crates.io/crates/parking)
[![Documentation](https://docs.rs/parking/badge.svg)](
https://docs.rs/parking)

Thread parking and unparking.

A `Parker` is in either the notified or unnotified state. The `park()` method blocks
the current thread until the `Parker` becomes notified and then puts it back into the unnotified
state. The `unpark()` method puts it into the notified state.

This API is similar to [`thread::park()`] and [`Thread::unpark()`] from the standard library.
The difference is that the state "token" managed by those functions is shared across an entire
thread, and anyone can call [`thread::current()`] to access it. If you use `park` and `unpark`,
but you also call a function that uses `park` and `unpark` internally, that function could
cause a deadlock by consuming a wakeup that was intended for you. The `Parker` object in this
crate avoids that problem by managing its own state, which isn't shared with unrelated callers.

[`thread::park()`]: https://doc.rust-lang.org/std/thread/fn.park.html
[`Thread::unpark()`]: https://doc.rust-lang.org/std/thread/struct.Thread.html#method.unpark
[`thread::current()`]: https://doc.rust-lang.org/std/thread/fn.current.html

## Examples

```rust
use std::thread;
use std::time::Duration;
use parking::Parker;

let p = Parker::new();
let u = p.unparker();

// Notify the parker.
u.unpark();

// Wakes up immediately because the parker is notified.
p.park();

thread::spawn(move || {
    thread::sleep(Duration::from_millis(500));
    u.unpark();
});

// Wakes up when `u.unpark()` notifies and then goes back into unnotified state.
p.park();
```

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
