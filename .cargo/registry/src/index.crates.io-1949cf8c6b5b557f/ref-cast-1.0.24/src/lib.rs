//! [![github]](https://github.com/dtolnay/ref-cast)&ensp;[![crates-io]](https://crates.io/crates/ref-cast)&ensp;[![docs-rs]](https://docs.rs/ref-cast)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! This crate provides a derive macro for generating safe conversions from `&T`
//! to `&U` where the struct `U` contains a single field of type `T`.
//!
//! # Basic example
//!
//! ```
//! use ref_cast::RefCast;
//!
//! #[derive(RefCast)]
//! #[repr(transparent)]
//! struct U(String);
//!
//! fn main() {
//!     let s = String::new();
//!
//!     // Safely cast from `&String` to `&U`.
//!     let u = U::ref_cast(&s);
//! }
//! ```
//!
//! Note that `#[repr(transparent)]` is required in order for the conversion to
//! be sound. The derive macro will refuse to compile if that is not present.
//!
//! # Realistic example
//!
//! Suppose we have a multidimensional array represented in a flat buffer in
//! row-major order for performance reasons, but we want to expose an indexing
//! operation that works in column-major order because it is more intuitive in
//! the context of our application.
//!
//! ```
//! const MAP_WIDTH: usize = 4;
//!
//! struct Tile(u8);
//!
//! struct TileMap {
//!     storage: Vec<Tile>,
//! }
//!
//! // `tilemap[x][y]` should give us `tilemap.storage[y * MAP_WIDTH + x]`.
//! ```
//!
//! The signature of the [`Index`] trait in Rust is such that the output is
//! forced to be borrowed from the type being indexed. So something like the
//! following is not going to work.
//!
//! [`Index`]: core::ops::Index
//!
//! ```
//! # const MAP_WIDTH: usize = 4;
//! #
//! # struct Tile(u8);
//! #
//! # struct TileMap {
//! #     storage: Vec<Tile>,
//! # }
//! #
//! struct Column<'a> {
//!     tilemap: &'a TileMap,
//!     x: usize,
//! }
//!
//! # mod index1 {
//! #     use super::{TileMap, Column, MAP_WIDTH};
//! #
//! #     trait Index<Idx> {
//! #         fn index(&self, idx: Idx) -> Column;
//! #     }
//! #
//! // Does not work! The output of Index must be a reference that is
//! // borrowed from self. Here the type Column is not a reference.
//! impl Index<usize> for TileMap {
//!     fn index(&self, x: usize) -> Column {
//!         assert!(x < MAP_WIDTH);
//!         Column { tilemap: self, x }
//!     }
//! }
//! # }
//!
//! # mod index2 {
//! #     use super::{Column, Tile, MAP_WIDTH};
//! #     use std::ops::Index;
//! #
//! impl<'a> Index<usize> for Column<'a> {
//!     # type Output = Tile;
//!     fn index(&self, y: usize) -> &Tile {
//!         &self.tilemap.storage[y * MAP_WIDTH + self.x]
//!     }
//! }
//! # }
//! #
//! # fn main() {}
//! ```
//!
//! Here is a working approach using `RefCast`.
//!
//! ```
//! # use ref_cast::RefCast;
//! # use std::ops::Index;
//! #
//! # const MAP_WIDTH: usize = 4;
//! #
//! # struct Tile(u8);
//! #
//! # struct TileMap {
//! #     storage: Vec<Tile>,
//! # }
//! #
//! #[derive(RefCast)]
//! #[repr(transparent)]
//! struct Strided([Tile]);
//!
//! // Implement `tilemap[x][y]` as `tilemap[x..][y * MAP_WIDTH]`.
//! impl Index<usize> for TileMap {
//!     type Output = Strided;
//!     fn index(&self, x: usize) -> &Self::Output {
//!         assert!(x < MAP_WIDTH);
//!         Strided::ref_cast(&self.storage[x..])
//!     }
//! }
//!
//! impl Index<usize> for Strided {
//!     type Output = Tile;
//!     fn index(&self, y: usize) -> &Self::Output {
//!         &self.0[y * MAP_WIDTH]
//!     }
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/ref-cast/1.0.24")]
#![no_std]
#![allow(
    clippy::extra_unused_type_parameters,
    clippy::let_underscore_untyped,
    clippy::manual_assert,
    clippy::missing_panics_doc,
    clippy::missing_safety_doc,
    clippy::module_name_repetitions,
    clippy::needless_doctest_main,
    clippy::needless_pass_by_value
)]

mod custom;
mod layout;
mod trivial;

pub use ref_cast_impl::{ref_cast_custom, RefCast, RefCastCustom};

/// Safely cast `&T` to `&U` where the struct `U` contains a single field of
/// type `T`.
///
/// ```
/// # use ref_cast::RefCast;
/// #
/// // `&String` can be cast to `&U`.
/// #[derive(RefCast)]
/// #[repr(transparent)]
/// struct U(String);
///
/// // `&T` can be cast to `&V<T>`.
/// #[derive(RefCast)]
/// #[repr(transparent)]
/// struct V<T> {
///     t: T,
/// }
/// ```
///
/// See the [crate-level documentation][crate] for usage examples!
pub trait RefCast {
    type From: ?Sized;
    fn ref_cast(from: &Self::From) -> &Self;
    fn ref_cast_mut(from: &mut Self::From) -> &mut Self;
}

// Not public API.
#[doc(hidden)]
pub mod __private {
    #[doc(hidden)]
    pub use crate::custom::{ref_cast_custom, CurrentCrate, RefCastCustom};
    #[doc(hidden)]
    pub use crate::layout::{assert_layout, Layout, LayoutUnsized};
    #[doc(hidden)]
    pub use crate::trivial::assert_trivial;
    #[doc(hidden)]
    pub use core::mem::transmute;
}
