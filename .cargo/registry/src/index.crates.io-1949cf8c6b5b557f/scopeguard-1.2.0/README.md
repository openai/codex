# scopeguard

Rust crate for a convenient RAII scope guard that will run a given closure when
it goes out of scope, even if the code between panics (assuming unwinding panic).

The `defer!` macro and `guard` are `no_std` compatible (require only `core`),
but the on unwinding / not on unwinding strategies require linking to `std`.
By default, the `use_std` crate feature is enabled. Disable the default features
for `no_std` support.

Please read the [API documentation here](https://docs.rs/scopeguard/).

Minimum supported Rust version: 1.20

[![build_status](https://github.com/bluss/scopeguard/actions/workflows/ci.yaml/badge.svg)](https://github.com/bluss/scopeguard/actions/workflows/ci.yaml)
[![crates](https://img.shields.io/crates/v/scopeguard.svg)](https://crates.io/crates/scopeguard)

## How to use

```rs
#[macro_use(defer)]
extern crate scopeguard;

use scopeguard::guard;

fn f() {
    defer! {
        println!("Called at return or panic");
    }
    panic!();
}

use std::fs::File;
use std::io::Write;

fn g() {
    let f = File::create("newfile.txt").unwrap();
    let mut file = guard(f, |f| {
        // write file at return or panic
        let _ = f.sync_all();
    });
    // access the file through the scope guard itself
    file.write_all(b"test me\n").unwrap();
}
```

## Recent Changes

- 1.2.0

  - Use ManuallyDrop instead of mem::forget in into_inner. (by @willtunnels)
  - Warn if the guard is not assigned to a variable and is dropped immediately
    instead of at the scope's end. (by @sergey-v-galtsev)

- 1.1.0

  - Change macros (`defer!`, `defer_on_success!` and `defer_on_unwind!`)
    to accept statements. (by @konsumlamm)

- 1.0.0

  - Change the closure type from `FnMut(&mut T)` to `FnOnce(T)`:
    Passing the inner value by value instead of a mutable reference is a
    breaking change, but allows the guard closure to consume it. (by @tormol)

  - Add `defer_on_success!`, `guard_on_success()` and `OnSuccess`
    strategy, which triggers when scope is exited *without* panic. It's the
    opposite to `defer_on_unwind!` / `guard_on_unwind()` / `OnUnwind`.

  - Add `ScopeGuard::into_inner()`, which "defuses" the guard and returns the
    guarded value. (by @tormol)

  - Implement `Sync` for guards with non-`Sync` closures.

  - Require Rust 1.20

- 0.3.3

  - Use `#[inline]` on a few more functions by @stjepang (#14)
  - Add examples to crate documentation

- 0.3.2

  - Add crate categories

- 0.3.1

  - Add `defer_on_unwind!`, `Strategy` trait
  - Rename `Guard` → `ScopeGuard`
  - Add `ScopeGuard::with_strategy`.
  - `ScopeGuard` now implements `Debug`.
  - Require Rust 1.11

- 0.2.0

  - Require Rust 1.6
  - Use `no_std` unconditionally
  - No other changes

- 0.1.2

  - Add macro `defer!`
