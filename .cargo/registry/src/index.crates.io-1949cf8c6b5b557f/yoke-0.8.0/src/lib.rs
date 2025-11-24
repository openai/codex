// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This crate provides [`Yoke<Y, C>`][Yoke], which allows one to "yoke" (attach) a zero-copy deserialized
//! object (say, a [`Cow<'a, str>`](alloc::borrow::Cow)) to the source it was deserialized from, (say, an [`Rc<[u8]>`](alloc::rc::Rc)),
//! known in this crate as a "cart", producing a type that looks like `Yoke<Cow<'static, str>, Rc<[u8]>>`
//! and can be moved around with impunity.
//!
//! Succinctly, this allows one to "erase" static lifetimes and turn them into dynamic ones, similarly
//! to how `dyn` allows one to "erase" static types and turn them into dynamic ones.
//!
//! Most of the time the yokeable `Y` type will be some kind of zero-copy deserializable
//! abstraction, potentially with an owned variant (like [`Cow`](alloc::borrow::Cow),
//! [`ZeroVec`](https://docs.rs/zerovec), or an aggregate containing such types), and the cart `C` will be some smart pointer like
//!   [`Box<T>`](alloc::boxed::Box), [`Rc<T>`](alloc::rc::Rc), or [`Arc<T>`](std::sync::Arc), potentially wrapped in an [`Option<T>`](Option).
//!
//! The key behind this crate is [`Yoke::get()`], where calling [`.get()`][Yoke::get] on a type like
//! `Yoke<Cow<'static, str>, _>` will get you a short-lived `&'a Cow<'a, str>`, restricted to the
//! lifetime of the borrow used during [`.get()`](Yoke::get). This is entirely safe since the `Cow` borrows from
//! the cart type `C`, which cannot be interfered with as long as the `Yoke` is borrowed by [`.get()`](Yoke::get).
//! [`.get()`](Yoke::get) protects access by essentially reifying the erased lifetime to a safe local one
//! when necessary.
//!
//! See the documentation of [`Yoke`] for more details.

// https://github.com/unicode-org/icu4x/blob/main/documents/process/boilerplate.md#library-annotations
#![cfg_attr(not(any(test, doc)), no_std)]
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::exhaustive_structs,
        clippy::exhaustive_enums,
        clippy::trivially_copy_pass_by_ref,
        missing_debug_implementations,
    )
)]
// The lifetimes here are important for safety and explicitly writing
// them out is good even when redundant
#![allow(clippy::needless_lifetimes)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod cartable_ptr;
pub mod either;
#[cfg(feature = "alloc")]
pub mod erased;
mod kinda_sorta_dangling;
mod macro_impls;
mod yoke;
mod yokeable;
#[cfg(feature = "zerofrom")]
mod zero_from;

#[cfg(feature = "derive")]
pub use yoke_derive::Yokeable;

pub use crate::yoke::{CloneableCart, Yoke};
pub use crate::yokeable::Yokeable;

#[cfg(feature = "zerofrom")]
use zerofrom::ZeroFrom;
