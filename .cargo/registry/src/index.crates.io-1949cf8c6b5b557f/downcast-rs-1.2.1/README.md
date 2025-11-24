# downcast-rs

[![Build status](https://img.shields.io/github/actions/workflow/status/marcianx/downcast-rs/main.yml?branch=master)](https://github.com/marcianx/downcast-rs/actions)
[![Latest version](https://img.shields.io/crates/v/downcast-rs.svg)](https://crates.io/crates/downcast-rs)
[![Documentation](https://docs.rs/downcast-rs/badge.svg)](https://docs.rs/downcast-rs)

Rust enums are great for types where all variations are known beforehand. But a
container of user-defined types requires an open-ended type like a **trait
object**. Some applications may want to cast these trait objects back to the
original concrete types to access additional functionality and performant
inlined implementations.

`downcast-rs` adds this downcasting support to trait objects using only safe
Rust. It supports **type parameters**, **associated types**, and **constraints**.

## Usage

Add the following to your `Cargo.toml`:

```toml
[dependencies]
downcast-rs = "1.2.1"
```

This crate is `no_std` compatible. To use it without `std`:

```toml
[dependencies]
downcast-rs = { version = "1.2.0", default-features = false }
```

To make a trait downcastable, make it extend either `downcast::Downcast` or
`downcast::DowncastSync` and invoke `impl_downcast!` on it as in the examples
below.

Since 1.2.0, the minimum supported Rust version is 1.36 due to needing stable access to alloc.

```rust
trait Trait: Downcast {}
impl_downcast!(Trait);

// Also supports downcasting `Arc`-ed trait objects by extending `DowncastSync`
// and starting `impl_downcast!` with `sync`.
trait TraitSync: DowncastSync {}
impl_downcast!(sync TraitSync);

// With type parameters.
trait TraitGeneric1<T>: Downcast {}
impl_downcast!(TraitGeneric1<T>);

// With associated types.
trait TraitGeneric2: Downcast { type G; type H; }
impl_downcast!(TraitGeneric2 assoc G, H);

// With constraints on types.
trait TraitGeneric3<T: Copy>: Downcast {
    type H: Clone;
}
impl_downcast!(TraitGeneric3<T> assoc H where T: Copy, H: Clone);

// With concrete types.
trait TraitConcrete1<T: Copy>: Downcast {}
impl_downcast!(concrete TraitConcrete1<u32>);

trait TraitConcrete2<T: Copy>: Downcast { type H; }
impl_downcast!(concrete TraitConcrete2<u32> assoc H=f64);
```

## Example without generics

```rust
// Import macro via `macro_use` pre-1.30.
#[macro_use]
extern crate downcast_rs;
use downcast_rs::DowncastSync;

// To create a trait with downcasting methods, extend `Downcast` or `DowncastSync`
// and run `impl_downcast!()` on the trait.
trait Base: DowncastSync {}
impl_downcast!(sync Base);  // `sync` => also produce `Arc` downcasts.

// Concrete types implementing Base.
#[derive(Debug)]
struct Foo(u32);
impl Base for Foo {}
#[derive(Debug)]
struct Bar(f64);
impl Base for Bar {}

fn main() {
    // Create a trait object.
    let mut base: Box<Base> = Box::new(Foo(42));

    // Try sequential downcasts.
    if let Some(foo) = base.downcast_ref::<Foo>() {
        assert_eq!(foo.0, 42);
    } else if let Some(bar) = base.downcast_ref::<Bar>() {
        assert_eq!(bar.0, 42.0);
    }

    assert!(base.is::<Foo>());

    // Fail to convert `Box<Base>` into `Box<Bar>`.
    let res = base.downcast::<Bar>();
    assert!(res.is_err());
    let base = res.unwrap_err();
    // Convert `Box<Base>` into `Box<Foo>`.
    assert_eq!(42, base.downcast::<Foo>().map_err(|_| "Shouldn't happen.").unwrap().0);

    // Also works with `Rc`.
    let mut rc: Rc<Base> = Rc::new(Foo(42));
    assert_eq!(42, rc.downcast_rc::<Foo>().map_err(|_| "Shouldn't happen.").unwrap().0);

    // Since this trait is `Sync`, it also supports `Arc` downcasts.
    let mut arc: Arc<Base> = Arc::new(Foo(42));
    assert_eq!(42, arc.downcast_arc::<Foo>().map_err(|_| "Shouldn't happen.").unwrap().0);
}
```

## Example with a generic trait with associated types and constraints

```rust
// Can call macro via namespace since rust 1.30.
extern crate downcast_rs;
use downcast_rs::Downcast;

// To create a trait with downcasting methods, extend `Downcast` or `DowncastSync`
// and run `impl_downcast!()` on the trait.
trait Base<T: Clone>: Downcast { type H: Copy; }
downcast_rs::impl_downcast!(Base<T> assoc H where T: Clone, H: Copy);
// or: impl_downcast!(concrete Base<u32> assoc H=f32)

// Concrete types implementing Base.
struct Foo(u32);
impl Base<u32> for Foo { type H = f32; }
struct Bar(f64);
impl Base<u32> for Bar { type H = f32; }

fn main() {
    // Create a trait object.
    let mut base: Box<Base<u32, H=f32>> = Box::new(Bar(42.0));

    // Try sequential downcasts.
    if let Some(foo) = base.downcast_ref::<Foo>() {
        assert_eq!(foo.0, 42);
    } else if let Some(bar) = base.downcast_ref::<Bar>() {
        assert_eq!(bar.0, 42.0);
    }

    assert!(base.is::<Bar>());
}
```

## Why no changes in a while?

This library is a thoroughly-tested boilerplate generator, is code complete, has
no unsafe, and is vanishingly unlikely to have any security issues to patch.

## License

Copyright 2020, Ashish Myles (maintainer) and contributors.
This software is dual-licensed under the [MIT](LICENSE-MIT) and
[Apache 2.0](LICENSE-APACHE) licenses.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
