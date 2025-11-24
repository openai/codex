# dtor

![Build Status](https://github.com/mmastrac/rust-ctor/actions/workflows/rust.yml/badge.svg)

`ctor` [![docs.rs](https://docs.rs/ctor/badge.svg)](https://docs.rs/ctor)
[![crates.io](https://img.shields.io/crates/v/ctor.svg)](https://crates.io/crates/ctor)

`dtor` [![docs.rs](https://docs.rs/dtor/badge.svg)](https://docs.rs/dtor)
[![crates.io](https://img.shields.io/crates/v/dtor.svg)](https://crates.io/crates/dtor)

Module teardown functions for Rust (like `__attribute__((destructor))` in C/C++)
for Linux, OSX, FreeBSD, NetBSD, Illumos, OpenBSD, DragonFlyBSD, Android, iOS,
WASM, and Windows.

# Examples

Print a message at shutdown time. Note that Rust may have shut down
some stdlib services at this time.

```rust,ignore
    #[dtor]
    unsafe fn shutdown() {
        // Using println or eprintln here will panic as Rust has shut down
        libc::printf("Shutting down!\n\0".as_ptr() as *const i8);
    }
```

# Under the Hood

The `#[dtor]` macro effectively creates a constructor that calls `libc::atexit`
with the provided function, ie roughly equivalent to:

```rust,ignore
    #[ctor]
    fn dtor_atexit() {
        libc::atexit(dtor);
    }
```
