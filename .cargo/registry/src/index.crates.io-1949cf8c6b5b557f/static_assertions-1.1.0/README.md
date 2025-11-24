[![Banner](https://raw.githubusercontent.com/nvzqz/static-assertions-rs/assets/Banner.png)](https://github.com/nvzqz/static-assertions-rs)

<div align="center">
    <a href="https://crates.io/crates/static_assertions">
        <img src="https://img.shields.io/crates/v/static_assertions.svg" alt="Crates.io">
        <img src="https://img.shields.io/crates/d/static_assertions.svg" alt="Downloads">
    </a>
    <a href="https://travis-ci.org/nvzqz/static-assertions-rs">
        <img src="https://travis-ci.org/nvzqz/static-assertions-rs.svg?branch=master" alt="Build Status">
    </a>
    <img src="https://img.shields.io/badge/rustc-^1.37.0-blue.svg" alt="rustc ^1.37.0">
    <br>
    <a href="https://www.patreon.com/nvzqz">
        <img src="https://c5.patreon.com/external/logo/become_a_patron_button.png" alt="Become a Patron!" height="35">
    </a>
    <a href="https://www.paypal.me/nvzqz">
        <img src="https://buymecoffee.intm.org/img/button-paypal-white.png" alt="Buy me a coffee" height="35">
    </a>
</div>

Compile-time assertions for Rust, brought to you by
[Nikolai Vazquez](https://twitter.com/NikolaiVazquez).

This library lets you ensure correct assumptions about constants, types, and
more. See the [docs] and [FAQ](#faq) for more info!

## Installation

This crate is available
[on crates.io](https://crates.io/crates/static_assertions) and can be used by
adding the following to your project's
[`Cargo.toml`](https://doc.rust-lang.org/cargo/reference/manifest.html):

```toml
[dependencies]
static_assertions = "1.1.0"
```

and this to your crate root (`main.rs` or `lib.rs`):

```rust
#[macro_use]
extern crate static_assertions;
```

## Usage

This crate exposes the following macros:
- [`assert_cfg!`]
- [`assert_eq_align!`]
- [`assert_eq_size!`]
- [`assert_eq_size_ptr!`]
- [`assert_eq_size_val!`]
- [`assert_fields!`]
- [`assert_impl_all!`]
- [`assert_impl_any!`]
- [`assert_impl_one!`]
- [`assert_not_impl_all!`]
- [`assert_not_impl_any!`]
- [`assert_obj_safe!`]
- [`assert_trait_sub_all!`]
- [`assert_trait_super_all!`]
- [`assert_type_eq_all!`]
- [`assert_type_ne_all!`]
- [`const_assert!`]
- [`const_assert_eq!`]
- [`const_assert_ne!`]

## FAQ

- **Q:** When would I want to use this?

  **A:** This library is useful for when wanting to ensure properties of
  constants, types, and traits.

  Basic examples:

  - With the release of 1.39, `str::len` can be called in a `const`
    context. Using [`const_assert!`], one can check that a string generated from
    elsewhere is of a given size:

    ```rust
    const DATA: &str = include_str!("path/to/string.txt");

    const_assert!(DATA.len() < 512);
    ```

  - Have a type that absolutely must implement certain traits? With
    [`assert_impl_all!`], one can ensure this:

    ```rust
    struct Foo {
        value: // ...
    }

    assert_impl_all!(Foo: Send, Sync);
    ```

- **Q:** How can I contribute?

  **A:** A couple of ways! You can:

  - Attempt coming up with some form of static analysis that you'd like to see
    implemented. Create a [new issue] and describe how you'd imagine your
    assertion to work, with example code to demonstrate.

  - Implement your own static assertion and create a [pull request].

  - Give feedback. What are some pain points? Where is it unpleasant?

  - Write docs. If you're familiar with how this library works, sharing your
    knowledge with the rest its users would be great!

- **Q:** Will this affect my compiled binary?

  **A:** Nope! There is zero runtime cost to using this because all checks are
  at compile-time, and so no code is emitted to run.

- **Q:** Will this affect my compile times?

  **A:** Likely not by anything perceivable. If this is a concern, this library
  can be put in `dev-dependencies`:

  ```toml
  [dev-dependencies]
  static_assertions = "1.1.0"
  ```

  and then assertions can be conditionally run behind `#[cfg(test)]`:

  ```rust
  #[cfg(test)]
  const_assert_eq!(MEANING_OF_LIFE, 42);
  ```

  However, the assertions will only be checked when running `cargo test`. This
  somewhat defeats the purpose of catching false static conditions up-front with
  a compilation failure.

- **Q:** What is `const _`?

  **A:** It's a way of creating an unnamed constant. This is used so that macros
  can be called from a global scope without requiring a scope-unique label. This
  library makes use of the side effects of evaluating the `const` expression.
  See the feature's
  [tracking issue](https://github.com/rust-lang/rust/issues/54912)
  and
  [issue #1](https://github.com/nvzqz/static-assertions-rs/issues/1)
  for more info.

## Changes

See [`CHANGELOG.md`](https://github.com/nvzqz/static-assertions-rs/blob/master/CHANGELOG.md)
for a complete list of what has changed from one version to another.

## License

This project is released under either:

- [MIT License](https://github.com/nvzqz/static-assertions-rs/blob/master/LICENSE-MIT)

- [Apache License (Version 2.0)](https://github.com/nvzqz/static-assertions-rs/blob/master/LICENSE-APACHE)

at your choosing.

[new issue]:    https://github.com/nvzqz/static-assertions-rs/issues/new
[pull request]: https://github.com/nvzqz/static-assertions-rs/pulls
[docs]:         https://docs.rs/static_assertions

[`assert_cfg!`]:             https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_cfg.html
[`assert_eq_align!`]:        https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_eq_align.html
[`assert_eq_size!`]:         https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_eq_size.html
[`assert_eq_size_ptr!`]:     https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_eq_size_ptr.html
[`assert_eq_size_val!`]:     https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_eq_size_val.html
[`assert_fields!`]:          https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_fields.html
[`assert_impl_all!`]:        https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_impl_all.html
[`assert_impl_any!`]:        https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_impl_any.html
[`assert_impl_one!`]:        https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_impl_one.html
[`assert_not_impl_all!`]:    https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_not_impl_all.html
[`assert_not_impl_any!`]:    https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_not_impl_any.html
[`assert_obj_safe!`]:        https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_obj_safe.html
[`assert_trait_sub_all!`]:   https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_trait_sub_all.html
[`assert_trait_super_all!`]: https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_trait_super_all.html
[`assert_type_eq_all!`]:     https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_type_eq_all.html
[`assert_type_ne_all!`]:     https://docs.rs/static_assertions/1.1.0/static_assertions/macro.assert_type_ne_all.html
[`const_assert!`]:           https://docs.rs/static_assertions/1.1.0/static_assertions/macro.const_assert.html
[`const_assert_eq!`]:        https://docs.rs/static_assertions/1.1.0/static_assertions/macro.const_assert_eq.html
[`const_assert_ne!`]:        https://docs.rs/static_assertions/1.1.0/static_assertions/macro.const_assert_ne.html
