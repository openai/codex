# rust-ctor

![Build Status](https://github.com/mmastrac/rust-ctor/actions/workflows/rust.yml/badge.svg)

`ctor` [![docs.rs](https://docs.rs/ctor/badge.svg)](https://docs.rs/ctor)
[![crates.io](https://img.shields.io/crates/v/ctor.svg)](https://crates.io/crates/ctor)

`dtor` [![docs.rs](https://docs.rs/dtor/badge.svg)](https://docs.rs/dtor)
[![crates.io](https://img.shields.io/crates/v/dtor.svg)](https://crates.io/crates/dtor)

Module initialization/teardown functions for Rust (like
`__attribute__((constructor))` in C/C++) for Linux, OSX, FreeBSD, NetBSD,
Illumos, OpenBSD, DragonFlyBSD, Android, iOS, WASM, and Windows.

This library currently requires **Rust > 1.56.0** at a minimum for edition
2021 support. Library versions 0.2.x should work for edition 2018, and 1.0
is planned to be released as 2021-only.

## Zero Dependency

As of `ctor 0.3.0+`, `ctor` has no dependencies (other than the
`ctor-proc-macro` crate). The proc macro in this crate calls into the
declarative macro that does the majority of the work.

## Support

This library works and is regularly tested on Linux, OSX, Windows, and FreeBSD,
with both `+crt-static` and `-crt-static` where possible. Other platforms are
supported but not tested as part of the automatic builds. This library will also
work as expected in both `bin` and `cdylib` outputs, ie: the `ctor` and `dtor`
will run at executable or library startup/shutdown respectively.

This library supports WASM targets, but the MSRV for this target is 1.85.

## Warnings

Rust's philosophy is that nothing happens before or after main and
this library explicitly subverts that. The code that runs in the `ctor`
and `dtor` functions should be careful to limit itself to `libc`
functions and code that does not rely on Rust's stdlib services.

For example, using stdout in a `dtor` function is a guaranteed panic. Consider
using the [`libc-print` crate](https://crates.io/crates/libc-print) for output
to stderr/stdout during `#[ctor]` and `#[dtor]` methods. Other issues
may involve signal processing or panic handling in that early code.

Some linker configurations may cause `#[ctor]` and `#[dtor]` functions to be
stripped from the final binary. The `used_linker` feature may prevent this, but
is not supported outside of nightly Rust. Often, a simple `use module_with_ctor`
is sufficient to ensure the linker does not strip the function.

On some platforms, unloading of shared libraries may not actually happen until
process exit, even if explicitly unloaded. The rules for this are arcane and
difficult to understand. For example, thread-local storage on OSX will affect
this (see [this
comment](https://github.com/rust-lang/rust/issues/28794#issuecomment-368693049)).

## Examples

Marks the function `foo` as a module constructor, called when a static
library is loaded or an executable is started:

```rust
    static INITED: AtomicBool = AtomicBool::new(false);

    #[ctor]
    fn foo() {
        INITED.store(true, Ordering::SeqCst);
    }
```

Creates a `HashMap` populated with strings when a static
library is loaded or an executable is started (new in `0.1.7`):

`static` items are equivalent to `std::sync::OnceLock`, but with
an automatic deref implementation and eager initialization at
startup time.

```rust
    #[ctor]
    /// This is an immutable static, evaluated at init time
    static STATIC_CTOR: HashMap<u32, &'static str> = {
        let mut m = HashMap::new();
        m.insert(0, "foo");
        m.insert(1, "bar");
        m.insert(2, "baz");
        m
    };
```

Print a message at shutdown time. Note that Rust may have shut down
some stdlib services at this time.

```rust
    #[dtor]
    unsafe fn shutdown() {
        // Using println or eprintln here will panic as Rust has shut down
        libc::printf("Shutting down!\n\0".as_ptr() as *const i8);
    }
```

## Under the Hood

The `#[ctor]` macro makes use of linker sections to ensure that a
function is run at startup time.

The above example translates into the following Rust code (approximately):

```rust
    #[used]
    #[cfg_attr(target_os = "linux", link_section = ".init_array")]
    #[cfg_attr(target_vendor = "apple", link_section = "__DATA,__mod_init_func,mod_init_funcs")]
    #[cfg_attr(target_os = "windows", link_section = ".CRT$XCU")]
    /* ... other platforms elided ... */
    static FOO: extern fn() = {
        extern fn foo() { /* ... */ };
        foo
    };
```

The `#[dtor]` macro effectively creates a constructor that calls `libc::atexit`
with the provided function, ie roughly equivalent to:

```rust
    #[ctor]
    fn dtor_atexit() {
        libc::atexit(dtor);
    }
```

## Inspiration

Idea inspired by [this
code](https://github.com/neon-bindings/neon/blob/2277e943a619579c144c1da543874f4a7ec39879/src/lib.rs#L42)
in the Neon project.
