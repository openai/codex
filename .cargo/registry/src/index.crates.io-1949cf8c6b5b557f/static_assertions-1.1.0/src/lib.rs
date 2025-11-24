//! [![Banner](https://raw.githubusercontent.com/nvzqz/static-assertions-rs/assets/Banner.png)](https://github.com/nvzqz/static-assertions-rs)
//!
//! <div align="center">
//!     <a href="https://crates.io/crates/static_assertions">
//!         <img src="https://img.shields.io/crates/d/static_assertions.svg" alt="Downloads">
//!     </a>
//!     <a href="https://travis-ci.org/nvzqz/static-assertions-rs">
//!         <img src="https://travis-ci.org/nvzqz/static-assertions-rs.svg?branch=master" alt="Build Status">
//!     </a>
//!     <img src="https://img.shields.io/badge/rustc-^1.37.0-blue.svg" alt="rustc ^1.37.0">
//!     <br><br>
//! </div>
//!
//! Assertions to ensure correct assumptions about constants, types, and more.
//!
//! _All_ checks provided by this crate are performed at [compile-time]. This
//! allows for finding errors quickly and early when it comes to ensuring
//! certain features or aspects of a codebase. These macros are especially
//! important when exposing a public API that requires types to be the same size
//! or implement certain traits.
//!
//! # Usage
//!
//! This crate is available [on crates.io][crate] and can be used by adding the
//! following to your project's [`Cargo.toml`]:
//!
//! ```toml
//! [dependencies]
//! static_assertions = "1.1.0"
//! ```
//!
//! and this to your crate root (`main.rs` or `lib.rs`):
//!
//! ```
//! #[macro_use]
//! extern crate static_assertions;
//! # fn main() {}
//! ```
//!
//! When using [Rust 2018 edition][2018], the following shorthand can help if
//! having `#[macro_use]` is undesirable.
//!
//! ```edition2018
//! extern crate static_assertions as sa;
//!
//! sa::const_assert!(true);
//! ```
//!
//! # Examples
//!
//! Very thorough examples are provided in the docs for
//! [each individual macro](#macros). Failure case examples are also documented.
//!
//! # Changes
//!
//! See [`CHANGELOG.md`](https://github.com/nvzqz/static-assertions-rs/blob/master/CHANGELOG.md)
//! for an exhaustive list of what has changed from one version to another.
//!
//! # Donate
//!
//! This project is made freely available (as in free beer), but unfortunately
//! not all beer is free! So, if you would like to buy me a beer (or coffee or
//! *more*), then consider supporting my work that's benefited your project
//! and thousands of others.
//!
//! <a href="https://www.patreon.com/nvzqz">
//!     <img src="https://c5.patreon.com/external/logo/become_a_patron_button.png" alt="Become a Patron!" height="35">
//! </a>
//! <a href="https://www.paypal.me/nvzqz">
//!     <img src="https://buymecoffee.intm.org/img/button-paypal-white.png" alt="Buy me a coffee" height="35">
//! </a>
//!
//! [Rust 1.37]: https://blog.rust-lang.org/2019/08/15/Rust-1.37.0.html
//! [2018]: https://blog.rust-lang.org/2018/12/06/Rust-1.31-and-rust-2018.html#rust-2018
//! [crate]: https://crates.io/crates/static_assertions
//! [compile-time]: https://en.wikipedia.org/wiki/Compile_time
//! [`Cargo.toml`]: https://doc.rust-lang.org/cargo/reference/manifest.html

#![doc(html_root_url = "https://docs.rs/static_assertions/1.1.0")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/nvzqz/static-assertions-rs/assets/Icon.png")]

#![no_std]

#![deny(unused_macros)]

#[doc(hidden)]
pub extern crate core as _core;

mod assert_cfg;
mod assert_eq_align;
mod assert_eq_size;
mod assert_fields;
mod assert_impl;
mod assert_obj_safe;
mod assert_trait;
mod assert_type;
mod const_assert;
