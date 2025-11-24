// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Collection types that allocate inside a [`Bump`] arena.
//!
//! [`Bump`]: ../struct.Bump.html

#![allow(deprecated)]

mod raw_vec;

pub mod vec;
pub use self::vec::Vec;

mod str;
pub mod string;
pub use self::string::String;

mod collect_in;
pub use collect_in::{CollectIn, FromIteratorIn};

// pub mod binary_heap;
// mod btree;
// pub mod linked_list;
// pub mod vec_deque;

// pub mod btree_map {
//     //! A map based on a B-Tree.
//     pub use super::btree::map::*;
// }

// pub mod btree_set {
//     //! A set based on a B-Tree.
//     pub use super::btree::set::*;
// }

// #[doc(no_inline)]
// pub use self::binary_heap::BinaryHeap;

// #[doc(no_inline)]
// pub use self::btree_map::BTreeMap;

// #[doc(no_inline)]
// pub use self::btree_set::BTreeSet;

// #[doc(no_inline)]
// pub use self::linked_list::LinkedList;

// #[doc(no_inline)]
// pub use self::vec_deque::VecDeque;

use crate::alloc::{AllocErr, LayoutErr};

/// Augments `AllocErr` with a `CapacityOverflow` variant.
#[derive(Clone, PartialEq, Eq, Debug)]
// #[unstable(feature = "try_reserve", reason = "new API", issue="48043")]
pub enum CollectionAllocErr {
    /// Error due to the computed capacity exceeding the collection's maximum
    /// (usually `isize::MAX` bytes).
    CapacityOverflow,
    /// Error due to the allocator (see the documentation for the [`AllocErr`] type).
    AllocErr,
}

// #[unstable(feature = "try_reserve", reason = "new API", issue="48043")]
impl From<AllocErr> for CollectionAllocErr {
    #[inline]
    fn from(AllocErr: AllocErr) -> Self {
        CollectionAllocErr::AllocErr
    }
}

// #[unstable(feature = "try_reserve", reason = "new API", issue="48043")]
impl From<LayoutErr> for CollectionAllocErr {
    #[inline]
    fn from(_: LayoutErr) -> Self {
        CollectionAllocErr::CapacityOverflow
    }
}

// /// An intermediate trait for specialization of `Extend`.
// #[doc(hidden)]
// trait SpecExtend<I: IntoIterator> {
//     /// Extends `self` with the contents of the given iterator.
//     fn spec_extend(&mut self, iter: I);
// }
