# proc-macro-crate


[![](https://docs.rs/proc-macro-crate/badge.svg)](https://docs.rs/proc-macro-crate/) [![](https://img.shields.io/crates/v/proc-macro-crate.svg)](https://crates.io/crates/proc-macro-crate) [![](https://img.shields.io/crates/d/proc-macro-crate.png)](https://crates.io/crates/proc-macro-crate) [![Build Status](https://travis-ci.org/bkchr/proc-macro-crate.png?branch=master)](https://travis-ci.org/bkchr/proc-macro-crate)

Providing support for `$crate` in procedural macros.

* [Introduction](#introduction)
* [Example](#example)
* [License](#license)

### Introduction

In `macro_rules!` `$crate` is used to get the path of the crate where a macro is declared in. In
procedural macros there is currently no easy way to get this path. A common hack is to import the
desired crate with a know name and use this. However, with rust edition 2018 and dropping
`extern crate` declarations from `lib.rs`, people start to rename crates in `Cargo.toml` directly.
However, this breaks importing the crate, as the proc-macro developer does not know the renamed
name of the crate that should be imported.

This crate provides a way to get the name of a crate, even if it renamed in `Cargo.toml`. For this
purpose a single function `crate_name` is provided. This function needs to be called in the context
of a proc-macro with the name of the desired crate. `CARGO_MANIFEST_DIR` will be used to find the
current active `Cargo.toml` and this `Cargo.toml` is searched for the desired crate.

### Example

```rust
use quote::quote;
use syn::Ident;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};

fn import_my_crate() {
    let found_crate = crate_name("my-crate").expect("my-crate is present in `Cargo.toml`");

    match found_crate {
        FoundCrate::Itself => quote!( crate::Something ),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!( #ident::Something )
        }
    };
}

```

### Edge cases

There are multiple edge cases when it comes to determining the correct crate. If you for example
import a crate as its own dependency, like this:

```toml
[package]
name = "my_crate"

[dev-dependencies]
my_crate = { version = "0.1", features = [ "test-feature" ] }
```

The crate will return `FoundCrate::Itself` and you will not be able to find the other instance
of your crate in `dev-dependencies`. Other similar cases are when one crate is imported multiple
times:

```toml
[package]
name = "my_crate"

[dependencies]
some-crate = { version = "0.5" }
some-crate-old = { package = "some-crate", version = "0.1" }
```

When searching for `some-crate` in this `Cargo.toml` it will return `FoundCrate::Name("some_old_crate")`,
aka the last definition of the crate in the `Cargo.toml`.

### License

Licensed under either of

 * [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)

 * [MIT license](https://opensource.org/licenses/MIT)

at your option.

License: MIT OR Apache-2.0
