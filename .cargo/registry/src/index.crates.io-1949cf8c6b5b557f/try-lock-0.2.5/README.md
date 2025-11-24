# TryLock

- [Crates.io](https://crates.io/crates/try-lock)
- [Docs](https://docs.rs/try-lock)

A light-weight lock guarded by an atomic boolean.

Most efficient when contention is low, acquiring the lock is a single atomic swap, and releasing it just 1 more atomic swap.

## Example

```rust
use std::sync::Arc;
use try_lock::TryLock;

// a thing we want to share
struct Widget {
    name: String,
}

// lock it up!
let widget1 = Arc::new(TryLock::new(Widget {
    name: "Spanner".into(),
}));

let widget2 = widget1.clone();


// mutate the widget
let mut locked = widget1.try_lock().expect("example isn't locked yet");
locked.name.push_str(" Bundle");

// hands off, buddy
let not_locked = widget2.try_lock();
assert!(not_locked.is_none(), "widget1 has the lock");

// ok, you can have it
drop(locked);

let locked2 = widget2.try_lock().expect("widget1 lock is released");

assert_eq!(locked2.name, "Spanner Bundle");
```

