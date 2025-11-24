# `derive_more`

[![Build Status](https://github.com/JelteF/derive_more/workflows/CI/badge.svg)](https://github.com/JelteF/derive_more/actions)
[![Latest Version](https://img.shields.io/crates/v/derive_more.svg)](https://crates.io/crates/derive_more)
[![Rust Documentation](https://docs.rs/derive_more/badge.svg)](https://docs.rs/derive_more)
[![GitHub license](https://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/JelteF/derive_more/master/LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rustc-1.75+-lightgray.svg)](https://blog.rust-lang.org/2023/12/28/Rust-1.75.0.html)
[![Unsafe Forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance)

Rust has lots of builtin traits that are implemented for its basic types, such
as `Add`, `Not`, `From` or `Display`.
However, when wrapping these types inside your own structs or enums you lose the
implementations of these traits and are required to recreate them.
This is especially annoying when your own structures are very simple, such as
when using the commonly advised newtype pattern (e.g. `MyInt(i32)`).

This library tries to remove these annoyances and the corresponding boilerplate code.
It does this by allowing you to derive lots of commonly used traits for both structs and enums.




## Example code

By using this library the following code just works:

```rust
use derive_more::{Add, Display, From, Into};

#[derive(PartialEq, From, Add)]
struct MyInt(i32);

#[derive(PartialEq, From, Into)]
struct Point2D {
    x: i32,
    y: i32,
}

#[derive(PartialEq, From, Add, Display)]
enum MyEnum {
    #[display("int: {_0}")]
    Int(i32),
    Uint(u32),
    #[display("nothing")]
    Nothing,
}

assert!(MyInt(11) == MyInt(5) + 6.into());
assert!((5, 6) == Point2D { x: 5, y: 6 }.into());
assert!(MyEnum::Int(15) == (MyEnum::Int(8) + 7.into()).unwrap());
assert!(MyEnum::Int(15).to_string() == "int: 15");
assert!(MyEnum::Uint(42).to_string() == "42");
assert!(MyEnum::Nothing.to_string() == "nothing");
```




## The derivable traits

Below are all the traits that you can derive using this library.
Some trait derivations are so similar that the further documentation will only show a single one
of them.
You can recognize these by the "-like" suffix in their name.
The trait name before that will be the only one that is used throughout the further
documentation.

It is important to understand what code gets generated when using one of the
derives from this crate.
That is why the links below explain what code gets generated for a trait for
each group from before.

You can use the [`cargo-expand`] utility to see the exact code that is generated
for your specific type.
This will show you your code with all macros and derives expanded.

**NOTE**: You still have to derive each trait separately. So `#[derive(Mul)]` doesn't
automatically derive `Div` as well. To derive both you should do `#[derive(Mul, Div)]`


### Conversion traits

These are traits that are used to convert automatically between types.

1. [`From`]
2. [`Into`]
3. [`FromStr`]
4. [`TryFrom`]
5. [`TryInto`]
6. [`IntoIterator`]
7. [`AsRef`], [`AsMut`]


### Formatting traits

These traits are used for converting a struct to a string in different ways.

1. [`Debug`]
2. [`Display`-like], contains `Display`, `Binary`, `Octal`, `LowerHex`,
   `UpperHex`, `LowerExp`, `UpperExp`, `Pointer`


### Error-handling traits

These traits are used to define error-types.

1. [`Error`]


### Operators

These are traits that can be used for operator overloading.

1. [`Index`]
2. [`Deref`]
3. [`Not`-like], contains `Not` and `Neg`
4. [`Add`-like], contains `Add`, `Sub`, `BitAnd`, `BitOr`, `BitXor`
5. [`Mul`-like], contains `Mul`, `Div`, `Rem`, `Shr` and `Shl`
6. [`Sum`-like], contains `Sum` and `Product`
7. [`IndexMut`]
8. [`DerefMut`]
9. [`AddAssign`-like], contains `AddAssign`, `SubAssign`, `BitAndAssign`,
   `BitOrAssign` and `BitXorAssign`
10. [`MulAssign`-like], contains `MulAssign`, `DivAssign`, `RemAssign`,
    `ShrAssign` and `ShlAssign`


### Static methods

These don't derive traits, but derive static methods instead.

1. [`Constructor`], this derives a `new` method that can be used as a constructor.
   This is very basic if you need more customization for your constructor, check
   out the [`derive-new`] crate.
2. [`IsVariant`], for each variant `foo` of an enum type, derives a `is_foo` method.
3. [`Unwrap`], for each variant `foo` of an enum type, derives an `unwrap_foo` method.
4. [`TryUnwrap`], for each variant `foo` of an enum type, derives an `try_unwrap_foo` method.


### Re-exports

This crate also re-exports all the standard library traits that it adds derives
for. So, both the `Display` derive and the `Display` trait will be in scope when
you add the following code:
```rust
use derive_more::Display; // also imports `core::fmt::Display`
```

For derive macros only, without the corresponding traits, do import them from
the `derive` module:
```rust
use derive_more::derive::Display; // imports macro only
```

#### Hygiene

For hygiene purposes, macros use `derive_more::*` absolute paths in their expansions.
This might introduce a trouble, if you want to re-export `derive_more` macros in your
own crate without using the `derive_more` as a direct dependency in downstream crates:
```rust,ignore
use my_lib::Display; // re-exported in `my_lib` crate

#[derive(Display)] // error: could not find `derive_more` in the list of imported crates
struct MyInt(i32);
```
In such case, you should re-export the `derive_more` module too:
```rust,ignore
use my_lib::{derive_more, Display}; // re-exported in `my_lib` crate

#[derive(Display)] // works fine now!
struct MyInt(i32);
```




## Installation

To avoid redundant compilation times, by default no derives are supported.
You have to enable each type of derive as a feature in `Cargo.toml`:
```toml
[dependencies]
# You can specify the types of derives that you need for less time spent
# compiling. For the full list of features see this crate its `Cargo.toml`.
derive_more = { version = "1", features = ["from", "add", "iterator"] }
```
```toml
[dependencies]
# If you don't care much about compilation times and simply want to have
# support for all the possible derives, you can use the "full" feature.
derive_more = { version = "1", features = ["full"] }
```
```toml
[dependencies]
# If you run in a `no_std` environment you should disable the default features,
# because the only default feature is the "std" feature.
# NOTE: You can combine this with "full" feature to get support for all the
#       possible derives in a `no_std` environment.
derive_more = { version = "1", default-features = false }
```

And this to the top of your Rust file:
```rust
// use the derives that you want in the file
use derive_more::{Add, Display, From};
```
If you're still using Rust 2015, add this instead:
```rust,edition2015
extern crate core;
#[macro_use]
extern crate derive_more;
# fn main() {} // omit wrapping statements above into `main()` in tests
```


## [MSRV] policy

This library requires Rust 1.75 or higher.

Changing [MSRV] (minimum supported Rust version) of this crate is treated as a **minor version change** in terms of [Semantic Versioning].
- So, if [MSRV] changes are **NOT concerning** for your project, just use the default [caret requirement]:
  ```toml
  [dependencies]
  derive_more = "1" # or "1.0", or "^1.0"
  ```
- However, if [MSRV] changes are concerning for your project, then use the [tilde requirement] to **pin to a specific minor version**:
  ```toml
  [dependencies]
  derive_more = "~1.0" # or "~1.0.0"
  ```




[`cargo-expand`]: https://github.com/dtolnay/cargo-expand
[`derive-new`]: https://github.com/nrc/derive-new

[`From`]: https://docs.rs/derive_more/latest/derive_more/derive.From.html
[`Into`]: https://docs.rs/derive_more/latest/derive_more/derive.Into.html
[`FromStr`]: https://docs.rs/derive_more/latest/derive_more/derive.FromStr.html
[`TryFrom`]: https://docs.rs/derive_more/latest/derive_more/derive.TryFrom.html
[`TryInto`]: https://docs.rs/derive_more/latest/derive_more/derive.TryInto.html
[`IntoIterator`]: https://docs.rs/derive_more/latest/derive_more/derive.IntoIterator.html
[`AsRef`]: https://docs.rs/derive_more/latest/derive_more/derive.AsRef.html
[`AsMut`]: https://docs.rs/derive_more/latest/derive_more/derive.AsMut.html

[`Debug`]: https://docs.rs/derive_more/latest/derive_more/derive.Debug.html
[`Display`-like]: https://docs.rs/derive_more/latest/derive_more/derive.Display.html

[`Error`]: https://docs.rs/derive_more/latest/derive_more/derive.Error.html

[`Index`]: https://docs.rs/derive_more/latest/derive_more/derive.Index.html
[`Deref`]: https://docs.rs/derive_more/latest/derive_more/derive.Deref.html
[`Not`-like]: https://docs.rs/derive_more/latest/derive_more/derive.Not.html
[`Add`-like]: https://docs.rs/derive_more/latest/derive_more/derive.Add.html
[`Mul`-like]: https://docs.rs/derive_more/latest/derive_more/derive.Mul.html
[`Sum`-like]: https://docs.rs/derive_more/latest/derive_more/derive.Sum.html
[`IndexMut`]: https://docs.rs/derive_more/latest/derive_more/derive.IndexMut.html
[`DerefMut`]: https://docs.rs/derive_more/latest/derive_more/derive.DerefMut.html
[`AddAssign`-like]: https://docs.rs/derive_more/latest/derive_more/derive.AddAssign.html
[`MulAssign`-like]: https://docs.rs/derive_more/latest/derive_more/derive.MulAssign.html

[`Constructor`]: https://docs.rs/derive_more/latest/derive_more/derive.Constructor.html
[`IsVariant`]: https://docs.rs/derive_more/latest/derive_more/derive.IsVariant.html
[`Unwrap`]: https://docs.rs/derive_more/latest/derive_more/derive.Unwrap.html
[`TryUnwrap`]: https://docs.rs/derive_more/latest/derive_more/derive.TryUnwrap.html

[caret requirement]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#caret-requirements
[tilde requirement]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#tilde-requirements
[MSRV]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field
[Semantic Versioning]: http://semver.org
