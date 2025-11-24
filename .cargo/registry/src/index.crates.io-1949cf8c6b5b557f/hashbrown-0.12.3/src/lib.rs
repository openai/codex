//! This crate is a Rust port of Google's high-performance [SwissTable] hash
//! map, adapted to make it a drop-in replacement for Rust's standard `HashMap`
//! and `HashSet` types.
//!
//! The original C++ version of [SwissTable] can be found [here], and this
//! [CppCon talk] gives an overview of how the algorithm works.
//!
//! [SwissTable]: https://abseil.io/blog/20180927-swisstables
//! [here]: https://github.com/abseil/abseil-cpp/blob/master/absl/container/internal/raw_hash_set.h
//! [CppCon talk]: https://www.youtube.com/watch?v=ncHmEUmJZf4

#![no_std]
#![cfg_attr(
    feature = "nightly",
    feature(
        test,
        core_intrinsics,
        dropck_eyepatch,
        min_specialization,
        extend_one,
        allocator_api,
        slice_ptr_get,
        nonnull_slice_from_raw_parts,
        maybe_uninit_array_assume_init,
        build_hasher_simple_hash_one
    )
)]
#![allow(
    clippy::doc_markdown,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::option_if_let_else,
    clippy::redundant_else,
    clippy::manual_map,
    clippy::missing_safety_doc,
    clippy::missing_errors_doc
)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

#[cfg(test)]
#[macro_use]
extern crate std;

#[cfg_attr(test, macro_use)]
extern crate alloc;

#[cfg(feature = "nightly")]
#[cfg(doctest)]
doc_comment::doctest!("../README.md");

#[macro_use]
mod macros;

#[cfg(feature = "raw")]
/// Experimental and unsafe `RawTable` API. This module is only available if the
/// `raw` feature is enabled.
pub mod raw {
    // The RawTable API is still experimental and is not properly documented yet.
    #[allow(missing_docs)]
    #[path = "mod.rs"]
    mod inner;
    pub use inner::*;

    #[cfg(feature = "rayon")]
    /// [rayon]-based parallel iterator types for hash maps.
    /// You will rarely need to interact with it directly unless you have need
    /// to name one of the iterator types.
    ///
    /// [rayon]: https://docs.rs/rayon/1.0/rayon
    pub mod rayon {
        pub use crate::external_trait_impls::rayon::raw::*;
    }
}
#[cfg(not(feature = "raw"))]
mod raw;

mod external_trait_impls;
mod map;
#[cfg(feature = "rustc-internal-api")]
mod rustc_entry;
mod scopeguard;
mod set;

pub mod hash_map {
    //! A hash map implemented with quadratic probing and SIMD lookup.
    pub use crate::map::*;

    #[cfg(feature = "rustc-internal-api")]
    pub use crate::rustc_entry::*;

    #[cfg(feature = "rayon")]
    /// [rayon]-based parallel iterator types for hash maps.
    /// You will rarely need to interact with it directly unless you have need
    /// to name one of the iterator types.
    ///
    /// [rayon]: https://docs.rs/rayon/1.0/rayon
    pub mod rayon {
        pub use crate::external_trait_impls::rayon::map::*;
    }
}
pub mod hash_set {
    //! A hash set implemented as a `HashMap` where the value is `()`.
    pub use crate::set::*;

    #[cfg(feature = "rayon")]
    /// [rayon]-based parallel iterator types for hash sets.
    /// You will rarely need to interact with it directly unless you have need
    /// to name one of the iterator types.
    ///
    /// [rayon]: https://docs.rs/rayon/1.0/rayon
    pub mod rayon {
        pub use crate::external_trait_impls::rayon::set::*;
    }
}

pub use crate::map::HashMap;
pub use crate::set::HashSet;

/// The error type for `try_reserve` methods.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TryReserveError {
    /// Error due to the computed capacity exceeding the collection's maximum
    /// (usually `isize::MAX` bytes).
    CapacityOverflow,

    /// The memory allocator returned an error
    AllocError {
        /// The layout of the allocation request that failed.
        layout: alloc::alloc::Layout,
    },
}

/// Wrapper around `Bump` which allows it to be used as an allocator for
/// `HashMap`, `HashSet` and `RawTable`.
///
/// `Bump` can be used directly without this wrapper on nightly if you enable
/// the `allocator-api` feature of the `bumpalo` crate.
#[cfg(feature = "bumpalo")]
#[derive(Clone, Copy, Debug)]
pub struct BumpWrapper<'a>(pub &'a bumpalo::Bump);

#[cfg(feature = "bumpalo")]
#[test]
fn test_bumpalo() {
    use bumpalo::Bump;
    let bump = Bump::new();
    let mut map = HashMap::new_in(BumpWrapper(&bump));
    map.insert(0, 1);
}
