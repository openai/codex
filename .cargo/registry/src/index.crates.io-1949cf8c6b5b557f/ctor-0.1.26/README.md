# rust-ctor

[![Build Status](https://api.travis-ci.com/mmastrac/rust-ctor.svg?branch=master)](https://travis-ci.com/mmastrac/rust-ctor)
[![docs.rs](https://docs.rs/ctor/badge.svg)](https://docs.rs/ctor)
[![crates.io](https://img.shields.io/crates/v/ctor.svg)](https://crates.io/crates/ctor)

Module initialization/teardown functions for Rust (like `__attribute__((constructor))` in C/C++) for Linux, OSX, FreeBSD, NetBSD, Illumos, OpenBSD, DragonFlyBSD, Android, iOS, and Windows.

This library currently requires **Rust > 1.31.0** at a minimum for the
procedural macro support.

Idea inspired by [this code](https://github.com/neon-bindings/neon/blob/2277e943a619579c144c1da543874f4a7ec39879/src/lib.rs#L42) in the Neon project.

## Support

This library works and [is regularly tested](https://travis-ci.org/mmastrac/rust-ctor)
on Linux, OSX and Windows, with both `+crt-static` and `-crt-static`. Other platforms are supported
but not tested as part of the automatic builds. This library will also work as expected in both
`bin` and `cdylib` outputs, ie: the `ctor` and `dtor` will run at executable or library 
startup/shutdown respectively.

## Warnings

Rust's philosophy is that nothing happens before or after main and 
this library explicitly subverts that. The code that runs in the `ctor`
and `dtor` functions should be careful to limit itself to `libc` 
functions and code that does not rely on Rust's stdlib services.

For example, using stdout in a `dtor` function is a guaranteed panic. Consider
using the [`libc-print` crate](https://crates.io/crates/libc-print) for output
to stderr/stdout during `#[ctor]` and `#[dtor]` methods. Other issues
may involve signal processing or panic handling in that early code.

In most cases, `sys_common::at_exit` is a better choice than `#[dtor]`. Caveat emptor!

On some platforms, unloading of shared libraries may not actually 
happen until process exit, even if explicitly unloaded. The rules for 
this are arcane and difficult to understand. For example, thread-local
storage on OSX will affect this (see [this comment](https://github.com/rust-lang/rust/issues/28794#issuecomment-368693049)).

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
    #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".init_array")]
    #[cfg_attr(target_os = "freebsd", link_section = ".init_array")]
    #[cfg_attr(target_os = "netbsd", link_section = ".init_array")]
    #[cfg_attr(target_os = "openbsd", link_section = ".init_array")]
    #[cfg_attr(target_os = "macos", link_section = "__DATA,__mod_init_func")]
    #[cfg_attr(target_os = "windows", link_section = ".CRT$XCU")]
    static FOO: extern fn() = {
        #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".text.startup")]
        extern fn foo() { /* ... */ };
        foo
    };
```

The `#[dtor]` macro effectively creates a constructor that calls `libc::atexit` with the provided function, ie roughly equivalent to:

```rust
    #[ctor]
    fn dtor_atexit() {
        libc::atexit(dtor);
    }
```
