# Signal-hook

[![Actions Status](https://github.com/vorner/signal-hook/workflows/test/badge.svg)](https://github.com/vorner/signal-hook/actions)
[![codecov](https://codecov.io/gh/vorner/signal-hook/branch/master/graph/badge.svg?token=3KA3R2D9fV)](https://codecov.io/gh/vorner/signal-hook)
[![docs](https://docs.rs/signal-hook/badge.svg)](https://docs.rs/signal-hook)

Library for safe and correct Unix signal handling in Rust.

Unix signals are inherently hard to handle correctly, for several reasons:

* They are a global resource. If a library wants to set its own signal handlers,
  it risks disturbing some other library. It is possible to chain the previous
  signal handler, but then it is impossible to remove the old signal handlers
  from the chains in any practical manner.
* They can be called from whatever thread, requiring synchronization. Also, as
  they can interrupt a thread at any time, making most handling race-prone.
* According to the POSIX standard, the set of functions one may call inside a
  signal handler is limited to very few of them. To highlight, mutexes (or other
  locking mechanisms) and memory allocation and deallocation are *not* allowed.

This library aims to solve some of the problems. It provides a global registry
of actions performed on arrival of signals. It is possible to register multiple
actions for the same signal and it is possible to remove the actions later on.
If there was a previous signal handler when the first action for a signal is
registered, it is chained (but the original one can't be removed).

Besides the basic registration of an arbitrary action, several helper actions
are provided to cover the needs of the most common use cases, available from
safe Rust.

For further details, see the [documentation](https://docs.rs/signal-hook).

## Example

(This likely does a lot more than you need in each individual application, it's
more of a show-case of what everything is possible, not of what you need to do
each time).

```rust
use std::io::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() -> Result<(), Error> {
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;
    while !term.load(Ordering::Relaxed) {
        // Do some time-limited stuff here
        // (if this could block forever, then there's no guarantee the signal will have any
        // effect).
    }
    Ok(())
}
```

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms
or conditions.
