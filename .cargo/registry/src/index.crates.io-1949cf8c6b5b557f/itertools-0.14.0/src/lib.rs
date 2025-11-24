#![warn(missing_docs, clippy::default_numeric_fallback)]
#![crate_name = "itertools"]
#![cfg_attr(not(feature = "use_std"), no_std)]
#![doc(test(attr(deny(warnings), allow(deprecated, unstable_name_collisions))))]

//! Extra iterator adaptors, functions and macros.
//!
//! To extend [`Iterator`] with methods in this crate, import
//! the [`Itertools`] trait:
//!
//! ```
//! # #[allow(unused_imports)]
//! use itertools::Itertools;
//! ```
//!
//! Now, new methods like [`interleave`](Itertools::interleave)
//! are available on all iterators:
//!
//! ```
//! use itertools::Itertools;
//!
//! let it = (1..3).interleave(vec![-1, -2]);
//! itertools::assert_equal(it, vec![1, -1, 2, -2]);
//! ```
//!
//! Most iterator methods are also provided as functions (with the benefit
//! that they convert parameters using [`IntoIterator`]):
//!
//! ```
//! use itertools::interleave;
//!
//! for elt in interleave(&[1, 2, 3], &[2, 3, 4]) {
//!     /* loop body */
//!     # let _ = elt;
//! }
//! ```
//!
//! ## Crate Features
//!
//! - `use_std`
//!   - Enabled by default.
//!   - Disable to compile itertools using `#![no_std]`. This disables
//!     any item that depend on allocations (see the `use_alloc` feature)
//!     and hash maps (like `unique`, `counts`, `into_grouping_map` and more).
//! - `use_alloc`
//!   - Enabled by default.
//!   - Enables any item that depend on allocations (like `chunk_by`,
//!     `kmerge`, `join` and many more).
//!
//! ## Rust Version
//!
//! This version of itertools requires Rust 1.63.0 or later.

#[cfg(not(feature = "use_std"))]
extern crate core as std;

#[cfg(feature = "use_alloc")]
extern crate alloc;

#[cfg(feature = "use_alloc")]
use alloc::{collections::VecDeque, string::String, vec::Vec};

pub use either::Either;

use core::borrow::Borrow;
use std::cmp::Ordering;
#[cfg(feature = "use_std")]
use std::collections::HashMap;
#[cfg(feature = "use_std")]
use std::collections::HashSet;
use std::fmt;
#[cfg(feature = "use_alloc")]
use std::fmt::Write;
#[cfg(feature = "use_std")]
use std::hash::Hash;
use std::iter::{once, IntoIterator};
#[cfg(feature = "use_alloc")]
type VecDequeIntoIter<T> = alloc::collections::vec_deque::IntoIter<T>;
#[cfg(feature = "use_alloc")]
type VecIntoIter<T> = alloc::vec::IntoIter<T>;
use std::iter::FromIterator;

#[macro_use]
mod impl_macros;

// for compatibility with no std and macros
#[doc(hidden)]
pub use std::iter as __std_iter;

/// The concrete iterator types.
pub mod structs {
    #[cfg(feature = "use_alloc")]
    pub use crate::adaptors::MultiProduct;
    pub use crate::adaptors::{
        Batching, Coalesce, Dedup, DedupBy, DedupByWithCount, DedupWithCount, FilterMapOk,
        FilterOk, Interleave, InterleaveShortest, MapInto, MapOk, Positions, Product, PutBack,
        TakeWhileRef, TupleCombinations, Update, WhileSome,
    };
    #[cfg(feature = "use_alloc")]
    pub use crate::combinations::{ArrayCombinations, Combinations};
    #[cfg(feature = "use_alloc")]
    pub use crate::combinations_with_replacement::CombinationsWithReplacement;
    pub use crate::cons_tuples_impl::ConsTuples;
    #[cfg(feature = "use_std")]
    pub use crate::duplicates_impl::{Duplicates, DuplicatesBy};
    pub use crate::exactly_one_err::ExactlyOneError;
    pub use crate::flatten_ok::FlattenOk;
    pub use crate::format::{Format, FormatWith};
    #[allow(deprecated)]
    #[cfg(feature = "use_alloc")]
    pub use crate::groupbylazy::GroupBy;
    #[cfg(feature = "use_alloc")]
    pub use crate::groupbylazy::{Chunk, ChunkBy, Chunks, Group, Groups, IntoChunks};
    #[cfg(feature = "use_std")]
    pub use crate::grouping_map::{GroupingMap, GroupingMapBy};
    pub use crate::intersperse::{Intersperse, IntersperseWith};
    #[cfg(feature = "use_alloc")]
    pub use crate::kmerge_impl::{KMerge, KMergeBy};
    pub use crate::merge_join::{Merge, MergeBy, MergeJoinBy};
    #[cfg(feature = "use_alloc")]
    pub use crate::multipeek_impl::MultiPeek;
    pub use crate::pad_tail::PadUsing;
    #[cfg(feature = "use_alloc")]
    pub use crate::peek_nth::PeekNth;
    pub use crate::peeking_take_while::PeekingTakeWhile;
    #[cfg(feature = "use_alloc")]
    pub use crate::permutations::Permutations;
    #[cfg(feature = "use_alloc")]
    pub use crate::powerset::Powerset;
    pub use crate::process_results_impl::ProcessResults;
    #[cfg(feature = "use_alloc")]
    pub use crate::put_back_n_impl::PutBackN;
    #[cfg(feature = "use_alloc")]
    pub use crate::rciter_impl::RcIter;
    pub use crate::repeatn::RepeatN;
    #[allow(deprecated)]
    pub use crate::sources::{Iterate, Unfold};
    pub use crate::take_while_inclusive::TakeWhileInclusive;
    #[cfg(feature = "use_alloc")]
    pub use crate::tee::Tee;
    pub use crate::tuple_impl::{CircularTupleWindows, TupleBuffer, TupleWindows, Tuples};
    #[cfg(feature = "use_std")]
    pub use crate::unique_impl::{Unique, UniqueBy};
    pub use crate::with_position::WithPosition;
    pub use crate::zip_eq_impl::ZipEq;
    pub use crate::zip_longest::ZipLongest;
    pub use crate::ziptuple::Zip;
}

/// Traits helpful for using certain `Itertools` methods in generic contexts.
pub mod traits {
    pub use crate::iter_index::IteratorIndex;
    pub use crate::tuple_impl::HomogeneousTuple;
}

pub use crate::concat_impl::concat;
pub use crate::cons_tuples_impl::cons_tuples;
pub use crate::diff::diff_with;
pub use crate::diff::Diff;
#[cfg(feature = "use_alloc")]
pub use crate::kmerge_impl::kmerge_by;
pub use crate::minmax::MinMaxResult;
pub use crate::peeking_take_while::PeekingNext;
pub use crate::process_results_impl::process_results;
pub use crate::repeatn::repeat_n;
#[allow(deprecated)]
pub use crate::sources::{iterate, unfold};
#[allow(deprecated)]
pub use crate::structs::*;
pub use crate::unziptuple::{multiunzip, MultiUnzip};
pub use crate::with_position::Position;
pub use crate::ziptuple::multizip;
mod adaptors;
mod either_or_both;
pub use crate::either_or_both::EitherOrBoth;
#[doc(hidden)]
pub mod free;
#[doc(inline)]
pub use crate::free::*;
#[cfg(feature = "use_alloc")]
mod combinations;
#[cfg(feature = "use_alloc")]
mod combinations_with_replacement;
mod concat_impl;
mod cons_tuples_impl;
mod diff;
#[cfg(feature = "use_std")]
mod duplicates_impl;
mod exactly_one_err;
#[cfg(feature = "use_alloc")]
mod extrema_set;
mod flatten_ok;
mod format;
#[cfg(feature = "use_alloc")]
mod group_map;
#[cfg(feature = "use_alloc")]
mod groupbylazy;
#[cfg(feature = "use_std")]
mod grouping_map;
mod intersperse;
mod iter_index;
#[cfg(feature = "use_alloc")]
mod k_smallest;
#[cfg(feature = "use_alloc")]
mod kmerge_impl;
#[cfg(feature = "use_alloc")]
mod lazy_buffer;
mod merge_join;
mod minmax;
#[cfg(feature = "use_alloc")]
mod multipeek_impl;
mod next_array;
mod pad_tail;
#[cfg(feature = "use_alloc")]
mod peek_nth;
mod peeking_take_while;
#[cfg(feature = "use_alloc")]
mod permutations;
#[cfg(feature = "use_alloc")]
mod powerset;
mod process_results_impl;
#[cfg(feature = "use_alloc")]
mod put_back_n_impl;
#[cfg(feature = "use_alloc")]
mod rciter_impl;
mod repeatn;
mod size_hint;
mod sources;
mod take_while_inclusive;
#[cfg(feature = "use_alloc")]
mod tee;
mod tuple_impl;
#[cfg(feature = "use_std")]
mod unique_impl;
mod unziptuple;
mod with_position;
mod zip_eq_impl;
mod zip_longest;
mod ziptuple;

#[macro_export]
/// Create an iterator over the “cartesian product” of iterators.
///
/// Iterator element type is like `(A, B, ..., E)` if formed
/// from iterators `(I, J, ..., M)` with element types `I::Item = A`, `J::Item = B`, etc.
///
/// ```
/// # use itertools::iproduct;
/// #
/// # fn main() {
/// // Iterate over the coordinates of a 4 x 4 x 4 grid
/// // from (0, 0, 0), (0, 0, 1), .., (0, 1, 0), (0, 1, 1), .. etc until (3, 3, 3)
/// for (i, j, k) in iproduct!(0..4, 0..4, 0..4) {
///    // ..
///    # let _ = (i, j, k);
/// }
/// # }
/// ```
macro_rules! iproduct {
    (@flatten $I:expr,) => (
        $I
    );
    (@flatten $I:expr, $J:expr, $($K:expr,)*) => (
        $crate::iproduct!(@flatten $crate::cons_tuples($crate::iproduct!($I, $J)), $($K,)*)
    );
    () => (
        $crate::__std_iter::once(())
    );
    ($I:expr $(,)?) => (
        $crate::__std_iter::Iterator::map(
            $crate::__std_iter::IntoIterator::into_iter($I),
            |elt| (elt,)
        )
    );
    ($I:expr, $J:expr $(,)?) => (
        $crate::Itertools::cartesian_product(
            $crate::__std_iter::IntoIterator::into_iter($I),
            $crate::__std_iter::IntoIterator::into_iter($J),
        )
    );
    ($I:expr, $J:expr, $($K:expr),+ $(,)?) => (
        $crate::iproduct!(@flatten $crate::iproduct!($I, $J), $($K,)+)
    );
}

#[macro_export]
/// Create an iterator running multiple iterators in lockstep.
///
/// The `izip!` iterator yields elements until any subiterator
/// returns `None`.
///
/// This is a version of the standard ``.zip()`` that's supporting more than
/// two iterators. The iterator element type is a tuple with one element
/// from each of the input iterators. Just like ``.zip()``, the iteration stops
/// when the shortest of the inputs reaches its end.
///
/// **Note:** The result of this macro is in the general case an iterator
/// composed of repeated `.zip()` and a `.map()`; it has an anonymous type.
/// The special cases of one and two arguments produce the equivalent of
/// `$a.into_iter()` and `$a.into_iter().zip($b)` respectively.
///
/// Prefer this macro `izip!()` over [`multizip`] for the performance benefits
/// of using the standard library `.zip()`.
///
/// ```
/// # use itertools::izip;
/// #
/// # fn main() {
///
/// // iterate over three sequences side-by-side
/// let mut results = [0, 0, 0, 0];
/// let inputs = [3, 7, 9, 6];
///
/// for (r, index, input) in izip!(&mut results, 0..10, &inputs) {
///     *r = index * 10 + input;
/// }
///
/// assert_eq!(results, [0 + 3, 10 + 7, 29, 36]);
/// # }
/// ```
macro_rules! izip {
    // @closure creates a tuple-flattening closure for .map() call. usage:
    // @closure partial_pattern => partial_tuple , rest , of , iterators
    // eg. izip!( @closure ((a, b), c) => (a, b, c) , dd , ee )
    ( @closure $p:pat => $tup:expr ) => {
        |$p| $tup
    };

    // The "b" identifier is a different identifier on each recursion level thanks to hygiene.
    ( @closure $p:pat => ( $($tup:tt)* ) , $_iter:expr $( , $tail:expr )* ) => {
        $crate::izip!(@closure ($p, b) => ( $($tup)*, b ) $( , $tail )*)
    };

    // unary
    ($first:expr $(,)*) => {
        $crate::__std_iter::IntoIterator::into_iter($first)
    };

    // binary
    ($first:expr, $second:expr $(,)*) => {
        $crate::__std_iter::Iterator::zip(
            $crate::__std_iter::IntoIterator::into_iter($first),
            $second,
        )
    };

    // n-ary where n > 2
    ( $first:expr $( , $rest:expr )* $(,)* ) => {
        {
            let iter = $crate::__std_iter::IntoIterator::into_iter($first);
            $(
                let iter = $crate::__std_iter::Iterator::zip(iter, $rest);
            )*
            $crate::__std_iter::Iterator::map(
                iter,
                $crate::izip!(@closure a => (a) $( , $rest )*)
            )
        }
    };
}

#[macro_export]
/// [Chain][`chain`] zero or more iterators together into one sequence.
///
/// The comma-separated arguments must implement [`IntoIterator`].
/// The final argument may be followed by a trailing comma.
///
/// [`chain`]: Iterator::chain
///
/// # Examples
///
/// Empty invocations of `chain!` expand to an invocation of [`std::iter::empty`]:
/// ```
/// use std::iter;
/// use itertools::chain;
///
/// let _: iter::Empty<()> = chain!();
/// let _: iter::Empty<i8> = chain!();
/// ```
///
/// Invocations of `chain!` with one argument expand to [`arg.into_iter()`](IntoIterator):
/// ```
/// use std::ops::Range;
/// use itertools::chain;
/// let _: <Range<_> as IntoIterator>::IntoIter = chain!(2..6,); // trailing comma optional!
/// let _:     <&[_] as IntoIterator>::IntoIter = chain!(&[2, 3, 4]);
/// ```
///
/// Invocations of `chain!` with multiple arguments [`.into_iter()`](IntoIterator) each
/// argument, and then [`chain`] them together:
/// ```
/// use std::{iter::*, slice};
/// use itertools::{assert_equal, chain};
///
/// // e.g., this:
/// let with_macro:  Chain<Chain<Once<_>, Take<Repeat<_>>>, slice::Iter<_>> =
///     chain![once(&0), repeat(&1).take(2), &[2, 3, 5],];
///
/// // ...is equivalent to this:
/// let with_method: Chain<Chain<Once<_>, Take<Repeat<_>>>, slice::Iter<_>> =
///     once(&0)
///         .chain(repeat(&1).take(2))
///         .chain(&[2, 3, 5]);
///
/// assert_equal(with_macro, with_method);
/// ```
macro_rules! chain {
    () => {
        $crate::__std_iter::empty()
    };
    ($first:expr $(, $rest:expr )* $(,)?) => {
        {
            let iter = $crate::__std_iter::IntoIterator::into_iter($first);
            $(
                let iter =
                    $crate::__std_iter::Iterator::chain(
                        iter,
                        $crate::__std_iter::IntoIterator::into_iter($rest));
            )*
            iter
        }
    };
}

/// An [`Iterator`] blanket implementation that provides extra adaptors and
/// methods.
///
/// This trait defines a number of methods. They are divided into two groups:
///
/// * *Adaptors* take an iterator and parameter as input, and return
///   a new iterator value. These are listed first in the trait. An example
///   of an adaptor is [`.interleave()`](Itertools::interleave)
///
/// * *Regular methods* are those that don't return iterators and instead
///   return a regular value of some other kind.
///   [`.next_tuple()`](Itertools::next_tuple) is an example and the first regular
///   method in the list.
pub trait Itertools: Iterator {
    // adaptors

    /// Alternate elements from two iterators until both have run out.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// This iterator is *fused*.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let it = (1..7).interleave(vec![-1, -2]);
    /// itertools::assert_equal(it, vec![1, -1, 2, -2, 3, 4, 5, 6]);
    /// ```
    fn interleave<J>(self, other: J) -> Interleave<Self, J::IntoIter>
    where
        J: IntoIterator<Item = Self::Item>,
        Self: Sized,
    {
        interleave(self, other)
    }

    /// Alternate elements from two iterators until at least one of them has run
    /// out.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let it = (1..7).interleave_shortest(vec![-1, -2]);
    /// itertools::assert_equal(it, vec![1, -1, 2, -2, 3]);
    /// ```
    fn interleave_shortest<J>(self, other: J) -> InterleaveShortest<Self, J::IntoIter>
    where
        J: IntoIterator<Item = Self::Item>,
        Self: Sized,
    {
        adaptors::interleave_shortest(self, other.into_iter())
    }

    /// An iterator adaptor to insert a particular value
    /// between each element of the adapted iterator.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// This iterator is *fused*.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// itertools::assert_equal((0..3).intersperse(8), vec![0, 8, 1, 8, 2]);
    /// ```
    fn intersperse(self, element: Self::Item) -> Intersperse<Self>
    where
        Self: Sized,
        Self::Item: Clone,
    {
        intersperse::intersperse(self, element)
    }

    /// An iterator adaptor to insert a particular value created by a function
    /// between each element of the adapted iterator.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// This iterator is *fused*.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let mut i = 10;
    /// itertools::assert_equal((0..3).intersperse_with(|| { i -= 1; i }), vec![0, 9, 1, 8, 2]);
    /// assert_eq!(i, 8);
    /// ```
    fn intersperse_with<F>(self, element: F) -> IntersperseWith<Self, F>
    where
        Self: Sized,
        F: FnMut() -> Self::Item,
    {
        intersperse::intersperse_with(self, element)
    }

    /// Returns an iterator over a subsection of the iterator.
    ///
    /// Works similarly to [`slice::get`](https://doc.rust-lang.org/std/primitive.slice.html#method.get).
    ///
    /// **Panics** for ranges `..=usize::MAX` and `0..=usize::MAX`.
    ///
    /// It's a generalisation of [`Iterator::take`] and [`Iterator::skip`],
    /// and uses these under the hood.
    /// Therefore, the resulting iterator is:
    /// - [`ExactSizeIterator`] if the adapted iterator is [`ExactSizeIterator`].
    /// - [`DoubleEndedIterator`] if the adapted iterator is [`DoubleEndedIterator`] and [`ExactSizeIterator`].
    ///
    /// # Unspecified Behavior
    /// The result of indexing with an exhausted [`core::ops::RangeInclusive`] is unspecified.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let vec = vec![3, 1, 4, 1, 5];
    ///
    /// let mut range: Vec<_> =
    ///         vec.iter().get(1..=3).copied().collect();
    /// assert_eq!(&range, &[1, 4, 1]);
    ///
    /// // It works with other types of ranges, too
    /// range = vec.iter().get(..2).copied().collect();
    /// assert_eq!(&range, &[3, 1]);
    ///
    /// range = vec.iter().get(0..1).copied().collect();
    /// assert_eq!(&range, &[3]);
    ///
    /// range = vec.iter().get(2..).copied().collect();
    /// assert_eq!(&range, &[4, 1, 5]);
    ///
    /// range = vec.iter().get(..=2).copied().collect();
    /// assert_eq!(&range, &[3, 1, 4]);
    ///
    /// range = vec.iter().get(..).copied().collect();
    /// assert_eq!(range, vec);
    /// ```
    fn get<R>(self, index: R) -> R::Output
    where
        Self: Sized,
        R: traits::IteratorIndex<Self>,
    {
        iter_index::get(self, index)
    }

    /// Create an iterator which iterates over both this and the specified
    /// iterator simultaneously, yielding pairs of two optional elements.
    ///
    /// This iterator is *fused*.
    ///
    /// As long as neither input iterator is exhausted yet, it yields two values
    /// via `EitherOrBoth::Both`.
    ///
    /// When the parameter iterator is exhausted, it only yields a value from the
    /// `self` iterator via `EitherOrBoth::Left`.
    ///
    /// When the `self` iterator is exhausted, it only yields a value from the
    /// parameter iterator via `EitherOrBoth::Right`.
    ///
    /// When both iterators return `None`, all further invocations of `.next()`
    /// will return `None`.
    ///
    /// Iterator element type is
    /// [`EitherOrBoth<Self::Item, J::Item>`](EitherOrBoth).
    ///
    /// ```rust
    /// use itertools::EitherOrBoth::{Both, Right};
    /// use itertools::Itertools;
    /// let it = (0..1).zip_longest(1..3);
    /// itertools::assert_equal(it, vec![Both(0, 1), Right(2)]);
    /// ```
    #[inline]
    fn zip_longest<J>(self, other: J) -> ZipLongest<Self, J::IntoIter>
    where
        J: IntoIterator,
        Self: Sized,
    {
        zip_longest::zip_longest(self, other.into_iter())
    }

    /// Create an iterator which iterates over both this and the specified
    /// iterator simultaneously, yielding pairs of elements.
    ///
    /// **Panics** if the iterators reach an end and they are not of equal
    /// lengths.
    #[inline]
    fn zip_eq<J>(self, other: J) -> ZipEq<Self, J::IntoIter>
    where
        J: IntoIterator,
        Self: Sized,
    {
        zip_eq(self, other)
    }

    /// A “meta iterator adaptor”. Its closure receives a reference to the
    /// iterator and may pick off as many elements as it likes, to produce the
    /// next iterator element.
    ///
    /// Iterator element type is `B`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // An adaptor that gathers elements in pairs
    /// let pit = (0..4).batching(|it| {
    ///            match it.next() {
    ///                None => None,
    ///                Some(x) => match it.next() {
    ///                    None => None,
    ///                    Some(y) => Some((x, y)),
    ///                }
    ///            }
    ///        });
    ///
    /// itertools::assert_equal(pit, vec![(0, 1), (2, 3)]);
    /// ```
    ///
    fn batching<B, F>(self, f: F) -> Batching<Self, F>
    where
        F: FnMut(&mut Self) -> Option<B>,
        Self: Sized,
    {
        adaptors::batching(self, f)
    }

    /// Return an *iterable* that can group iterator elements.
    /// Consecutive elements that map to the same key (“runs”), are assigned
    /// to the same group.
    ///
    /// `ChunkBy` is the storage for the lazy grouping operation.
    ///
    /// If the groups are consumed in order, or if each group's iterator is
    /// dropped without keeping it around, then `ChunkBy` uses no
    /// allocations.  It needs allocations only if several group iterators
    /// are alive at the same time.
    ///
    /// This type implements [`IntoIterator`] (it is **not** an iterator
    /// itself), because the group iterators need to borrow from this
    /// value. It should be stored in a local variable or temporary and
    /// iterated.
    ///
    /// Iterator element type is `(K, Group)`: the group's key and the
    /// group iterator.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // chunk data into runs of larger than zero or not.
    /// let data = vec![1, 3, -2, -2, 1, 0, 1, 2];
    /// // chunks:     |---->|------>|--------->|
    ///
    /// // Note: The `&` is significant here, `ChunkBy` is iterable
    /// // only by reference. You can also call `.into_iter()` explicitly.
    /// let mut data_grouped = Vec::new();
    /// for (key, chunk) in &data.into_iter().chunk_by(|elt| *elt >= 0) {
    ///     data_grouped.push((key, chunk.collect()));
    /// }
    /// assert_eq!(data_grouped, vec![(true, vec![1, 3]), (false, vec![-2, -2]), (true, vec![1, 0, 1, 2])]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn chunk_by<K, F>(self, key: F) -> ChunkBy<K, Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> K,
        K: PartialEq,
    {
        groupbylazy::new(self, key)
    }

    /// See [`.chunk_by()`](Itertools::chunk_by).
    #[deprecated(note = "Use .chunk_by() instead", since = "0.13.0")]
    #[cfg(feature = "use_alloc")]
    fn group_by<K, F>(self, key: F) -> ChunkBy<K, Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> K,
        K: PartialEq,
    {
        self.chunk_by(key)
    }

    /// Return an *iterable* that can chunk the iterator.
    ///
    /// Yield subiterators (chunks) that each yield a fixed number elements,
    /// determined by `size`. The last chunk will be shorter if there aren't
    /// enough elements.
    ///
    /// `IntoChunks` is based on `ChunkBy`: it is iterable (implements
    /// `IntoIterator`, **not** `Iterator`), and it only buffers if several
    /// chunk iterators are alive at the same time.
    ///
    /// Iterator element type is `Chunk`, each chunk's iterator.
    ///
    /// **Panics** if `size` is 0.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec![1, 1, 2, -2, 6, 0, 3, 1];
    /// //chunk size=3 |------->|-------->|--->|
    ///
    /// // Note: The `&` is significant here, `IntoChunks` is iterable
    /// // only by reference. You can also call `.into_iter()` explicitly.
    /// for chunk in &data.into_iter().chunks(3) {
    ///     // Check that the sum of each chunk is 4.
    ///     assert_eq!(4, chunk.sum());
    /// }
    /// ```
    #[cfg(feature = "use_alloc")]
    fn chunks(self, size: usize) -> IntoChunks<Self>
    where
        Self: Sized,
    {
        assert!(size != 0);
        groupbylazy::new_chunks(self, size)
    }

    /// Return an iterator over all contiguous windows producing tuples of
    /// a specific size (up to 12).
    ///
    /// `tuple_windows` clones the iterator elements so that they can be
    /// part of successive windows, this makes it most suited for iterators
    /// of references and other values that are cheap to copy.
    ///
    /// ```
    /// use itertools::Itertools;
    /// let mut v = Vec::new();
    ///
    /// // pairwise iteration
    /// for (a, b) in (1..5).tuple_windows() {
    ///     v.push((a, b));
    /// }
    /// assert_eq!(v, vec![(1, 2), (2, 3), (3, 4)]);
    ///
    /// let mut it = (1..5).tuple_windows();
    /// assert_eq!(Some((1, 2, 3)), it.next());
    /// assert_eq!(Some((2, 3, 4)), it.next());
    /// assert_eq!(None, it.next());
    ///
    /// // this requires a type hint
    /// let it = (1..5).tuple_windows::<(_, _, _)>();
    /// itertools::assert_equal(it, vec![(1, 2, 3), (2, 3, 4)]);
    ///
    /// // you can also specify the complete type
    /// use itertools::TupleWindows;
    /// use std::ops::Range;
    ///
    /// let it: TupleWindows<Range<u32>, (u32, u32, u32)> = (1..5).tuple_windows();
    /// itertools::assert_equal(it, vec![(1, 2, 3), (2, 3, 4)]);
    /// ```
    fn tuple_windows<T>(self) -> TupleWindows<Self, T>
    where
        Self: Sized + Iterator<Item = T::Item>,
        T: traits::HomogeneousTuple,
        T::Item: Clone,
    {
        tuple_impl::tuple_windows(self)
    }

    /// Return an iterator over all windows, wrapping back to the first
    /// elements when the window would otherwise exceed the length of the
    /// iterator, producing tuples of a specific size (up to 12).
    ///
    /// `circular_tuple_windows` clones the iterator elements so that they can be
    /// part of successive windows, this makes it most suited for iterators
    /// of references and other values that are cheap to copy.
    ///
    /// ```
    /// use itertools::Itertools;
    /// let mut v = Vec::new();
    /// for (a, b) in (1..5).circular_tuple_windows() {
    ///     v.push((a, b));
    /// }
    /// assert_eq!(v, vec![(1, 2), (2, 3), (3, 4), (4, 1)]);
    ///
    /// let mut it = (1..5).circular_tuple_windows();
    /// assert_eq!(Some((1, 2, 3)), it.next());
    /// assert_eq!(Some((2, 3, 4)), it.next());
    /// assert_eq!(Some((3, 4, 1)), it.next());
    /// assert_eq!(Some((4, 1, 2)), it.next());
    /// assert_eq!(None, it.next());
    ///
    /// // this requires a type hint
    /// let it = (1..5).circular_tuple_windows::<(_, _, _)>();
    /// itertools::assert_equal(it, vec![(1, 2, 3), (2, 3, 4), (3, 4, 1), (4, 1, 2)]);
    /// ```
    fn circular_tuple_windows<T>(self) -> CircularTupleWindows<Self, T>
    where
        Self: Sized + Clone + Iterator<Item = T::Item> + ExactSizeIterator,
        T: tuple_impl::TupleCollect + Clone,
        T::Item: Clone,
    {
        tuple_impl::circular_tuple_windows(self)
    }
    /// Return an iterator that groups the items in tuples of a specific size
    /// (up to 12).
    ///
    /// See also the method [`.next_tuple()`](Itertools::next_tuple).
    ///
    /// ```
    /// use itertools::Itertools;
    /// let mut v = Vec::new();
    /// for (a, b) in (1..5).tuples() {
    ///     v.push((a, b));
    /// }
    /// assert_eq!(v, vec![(1, 2), (3, 4)]);
    ///
    /// let mut it = (1..7).tuples();
    /// assert_eq!(Some((1, 2, 3)), it.next());
    /// assert_eq!(Some((4, 5, 6)), it.next());
    /// assert_eq!(None, it.next());
    ///
    /// // this requires a type hint
    /// let it = (1..7).tuples::<(_, _, _)>();
    /// itertools::assert_equal(it, vec![(1, 2, 3), (4, 5, 6)]);
    ///
    /// // you can also specify the complete type
    /// use itertools::Tuples;
    /// use std::ops::Range;
    ///
    /// let it: Tuples<Range<u32>, (u32, u32, u32)> = (1..7).tuples();
    /// itertools::assert_equal(it, vec![(1, 2, 3), (4, 5, 6)]);
    /// ```
    ///
    /// See also [`Tuples::into_buffer`].
    fn tuples<T>(self) -> Tuples<Self, T>
    where
        Self: Sized + Iterator<Item = T::Item>,
        T: traits::HomogeneousTuple,
    {
        tuple_impl::tuples(self)
    }

    /// Split into an iterator pair that both yield all elements from
    /// the original iterator.
    ///
    /// **Note:** If the iterator is clonable, prefer using that instead
    /// of using this method. Cloning is likely to be more efficient.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// ```
    /// use itertools::Itertools;
    /// let xs = vec![0, 1, 2, 3];
    ///
    /// let (mut t1, t2) = xs.into_iter().tee();
    /// itertools::assert_equal(t1.next(), Some(0));
    /// itertools::assert_equal(t2, 0..4);
    /// itertools::assert_equal(t1, 1..4);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn tee(self) -> (Tee<Self>, Tee<Self>)
    where
        Self: Sized,
        Self::Item: Clone,
    {
        tee::new(self)
    }

    /// Convert each item of the iterator using the [`Into`] trait.
    ///
    /// ```rust
    /// use itertools::Itertools;
    ///
    /// (1i32..42i32).map_into::<f64>().collect_vec();
    /// ```
    fn map_into<R>(self) -> MapInto<Self, R>
    where
        Self: Sized,
        Self::Item: Into<R>,
    {
        adaptors::map_into(self)
    }

    /// Return an iterator adaptor that applies the provided closure
    /// to every `Result::Ok` value. `Result::Err` values are
    /// unchanged.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let input = vec![Ok(41), Err(false), Ok(11)];
    /// let it = input.into_iter().map_ok(|i| i + 1);
    /// itertools::assert_equal(it, vec![Ok(42), Err(false), Ok(12)]);
    /// ```
    fn map_ok<F, T, U, E>(self, f: F) -> MapOk<Self, F>
    where
        Self: Iterator<Item = Result<T, E>> + Sized,
        F: FnMut(T) -> U,
    {
        adaptors::map_ok(self, f)
    }

    /// Return an iterator adaptor that filters every `Result::Ok`
    /// value with the provided closure. `Result::Err` values are
    /// unchanged.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let input = vec![Ok(22), Err(false), Ok(11)];
    /// let it = input.into_iter().filter_ok(|&i| i > 20);
    /// itertools::assert_equal(it, vec![Ok(22), Err(false)]);
    /// ```
    fn filter_ok<F, T, E>(self, f: F) -> FilterOk<Self, F>
    where
        Self: Iterator<Item = Result<T, E>> + Sized,
        F: FnMut(&T) -> bool,
    {
        adaptors::filter_ok(self, f)
    }

    /// Return an iterator adaptor that filters and transforms every
    /// `Result::Ok` value with the provided closure. `Result::Err`
    /// values are unchanged.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let input = vec![Ok(22), Err(false), Ok(11)];
    /// let it = input.into_iter().filter_map_ok(|i| if i > 20 { Some(i * 2) } else { None });
    /// itertools::assert_equal(it, vec![Ok(44), Err(false)]);
    /// ```
    fn filter_map_ok<F, T, U, E>(self, f: F) -> FilterMapOk<Self, F>
    where
        Self: Iterator<Item = Result<T, E>> + Sized,
        F: FnMut(T) -> Option<U>,
    {
        adaptors::filter_map_ok(self, f)
    }

    /// Return an iterator adaptor that flattens every `Result::Ok` value into
    /// a series of `Result::Ok` values. `Result::Err` values are unchanged.
    ///
    /// This is useful when you have some common error type for your crate and
    /// need to propagate it upwards, but the `Result::Ok` case needs to be flattened.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let input = vec![Ok(0..2), Err(false), Ok(2..4)];
    /// let it = input.iter().cloned().flatten_ok();
    /// itertools::assert_equal(it.clone(), vec![Ok(0), Ok(1), Err(false), Ok(2), Ok(3)]);
    ///
    /// // This can also be used to propagate errors when collecting.
    /// let output_result: Result<Vec<i32>, bool> = it.collect();
    /// assert_eq!(output_result, Err(false));
    /// ```
    fn flatten_ok<T, E>(self) -> FlattenOk<Self, T, E>
    where
        Self: Iterator<Item = Result<T, E>> + Sized,
        T: IntoIterator,
    {
        flatten_ok::flatten_ok(self)
    }

    /// “Lift” a function of the values of the current iterator so as to process
    /// an iterator of `Result` values instead.
    ///
    /// `processor` is a closure that receives an adapted version of the iterator
    /// as the only argument — the adapted iterator produces elements of type `T`,
    /// as long as the original iterator produces `Ok` values.
    ///
    /// If the original iterable produces an error at any point, the adapted
    /// iterator ends and it will return the error iself.
    ///
    /// Otherwise, the return value from the closure is returned wrapped
    /// inside `Ok`.
    ///
    /// # Example
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// type Item = Result<i32, &'static str>;
    ///
    /// let first_values: Vec<Item> = vec![Ok(1), Ok(0), Ok(3)];
    /// let second_values: Vec<Item> = vec![Ok(2), Ok(1), Err("overflow")];
    ///
    /// // “Lift” the iterator .max() method to work on the Ok-values.
    /// let first_max = first_values.into_iter().process_results(|iter| iter.max().unwrap_or(0));
    /// let second_max = second_values.into_iter().process_results(|iter| iter.max().unwrap_or(0));
    ///
    /// assert_eq!(first_max, Ok(3));
    /// assert!(second_max.is_err());
    /// ```
    fn process_results<F, T, E, R>(self, processor: F) -> Result<R, E>
    where
        Self: Iterator<Item = Result<T, E>> + Sized,
        F: FnOnce(ProcessResults<Self, E>) -> R,
    {
        process_results(self, processor)
    }

    /// Return an iterator adaptor that merges the two base iterators in
    /// ascending order.  If both base iterators are sorted (ascending), the
    /// result is sorted.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a = (0..11).step_by(3);
    /// let b = (0..11).step_by(5);
    /// let it = a.merge(b);
    /// itertools::assert_equal(it, vec![0, 0, 3, 5, 6, 9, 10]);
    /// ```
    fn merge<J>(self, other: J) -> Merge<Self, J::IntoIter>
    where
        Self: Sized,
        Self::Item: PartialOrd,
        J: IntoIterator<Item = Self::Item>,
    {
        merge(self, other)
    }

    /// Return an iterator adaptor that merges the two base iterators in order.
    /// This is much like [`.merge()`](Itertools::merge) but allows for a custom ordering.
    ///
    /// This can be especially useful for sequences of tuples.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a = (0..).zip("bc".chars());
    /// let b = (0..).zip("ad".chars());
    /// let it = a.merge_by(b, |x, y| x.1 <= y.1);
    /// itertools::assert_equal(it, vec![(0, 'a'), (0, 'b'), (1, 'c'), (1, 'd')]);
    /// ```
    fn merge_by<J, F>(self, other: J, is_first: F) -> MergeBy<Self, J::IntoIter, F>
    where
        Self: Sized,
        J: IntoIterator<Item = Self::Item>,
        F: FnMut(&Self::Item, &Self::Item) -> bool,
    {
        merge_join::merge_by_new(self, other, is_first)
    }

    /// Create an iterator that merges items from both this and the specified
    /// iterator in ascending order.
    ///
    /// The function can either return an `Ordering` variant or a boolean.
    ///
    /// If `cmp_fn` returns `Ordering`,
    /// it chooses whether to pair elements based on the `Ordering` returned by the
    /// specified compare function. At any point, inspecting the tip of the
    /// iterators `I` and `J` as items `i` of type `I::Item` and `j` of type
    /// `J::Item` respectively, the resulting iterator will:
    ///
    /// - Emit `EitherOrBoth::Left(i)` when `i < j`,
    ///   and remove `i` from its source iterator
    /// - Emit `EitherOrBoth::Right(j)` when `i > j`,
    ///   and remove `j` from its source iterator
    /// - Emit `EitherOrBoth::Both(i, j)` when  `i == j`,
    ///   and remove both `i` and `j` from their respective source iterators
    ///
    /// ```
    /// use itertools::Itertools;
    /// use itertools::EitherOrBoth::{Left, Right, Both};
    ///
    /// let a = vec![0, 2, 4, 6, 1].into_iter();
    /// let b = (0..10).step_by(3);
    ///
    /// itertools::assert_equal(
    ///     // This performs a diff in the style of the Unix command comm(1),
    ///     // generalized to arbitrary types rather than text.
    ///     a.merge_join_by(b, Ord::cmp),
    ///     vec![Both(0, 0), Left(2), Right(3), Left(4), Both(6, 6), Left(1), Right(9)]
    /// );
    /// ```
    ///
    /// If `cmp_fn` returns `bool`,
    /// it chooses whether to pair elements based on the boolean returned by the
    /// specified function. At any point, inspecting the tip of the
    /// iterators `I` and `J` as items `i` of type `I::Item` and `j` of type
    /// `J::Item` respectively, the resulting iterator will:
    ///
    /// - Emit `Either::Left(i)` when `true`,
    ///   and remove `i` from its source iterator
    /// - Emit `Either::Right(j)` when `false`,
    ///   and remove `j` from its source iterator
    ///
    /// It is similar to the `Ordering` case if the first argument is considered
    /// "less" than the second argument.
    ///
    /// ```
    /// use itertools::Itertools;
    /// use itertools::Either::{Left, Right};
    ///
    /// let a = vec![0, 2, 4, 6, 1].into_iter();
    /// let b = (0..10).step_by(3);
    ///
    /// itertools::assert_equal(
    ///     a.merge_join_by(b, |i, j| i <= j),
    ///     vec![Left(0), Right(0), Left(2), Right(3), Left(4), Left(6), Left(1), Right(6), Right(9)]
    /// );
    /// ```
    #[inline]
    #[doc(alias = "comm")]
    fn merge_join_by<J, F, T>(self, other: J, cmp_fn: F) -> MergeJoinBy<Self, J::IntoIter, F>
    where
        J: IntoIterator,
        F: FnMut(&Self::Item, &J::Item) -> T,
        Self: Sized,
    {
        merge_join_by(self, other, cmp_fn)
    }

    /// Return an iterator adaptor that flattens an iterator of iterators by
    /// merging them in ascending order.
    ///
    /// If all base iterators are sorted (ascending), the result is sorted.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a = (0..6).step_by(3);
    /// let b = (1..6).step_by(3);
    /// let c = (2..6).step_by(3);
    /// let it = vec![a, b, c].into_iter().kmerge();
    /// itertools::assert_equal(it, vec![0, 1, 2, 3, 4, 5]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn kmerge(self) -> KMerge<<Self::Item as IntoIterator>::IntoIter>
    where
        Self: Sized,
        Self::Item: IntoIterator,
        <Self::Item as IntoIterator>::Item: PartialOrd,
    {
        kmerge(self)
    }

    /// Return an iterator adaptor that flattens an iterator of iterators by
    /// merging them according to the given closure.
    ///
    /// The closure `first` is called with two elements *a*, *b* and should
    /// return `true` if *a* is ordered before *b*.
    ///
    /// If all base iterators are sorted according to `first`, the result is
    /// sorted.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a = vec![-1f64, 2., 3., -5., 6., -7.];
    /// let b = vec![0., 2., -4.];
    /// let mut it = vec![a, b].into_iter().kmerge_by(|a, b| a.abs() < b.abs());
    /// assert_eq!(it.next(), Some(0.));
    /// assert_eq!(it.last(), Some(-7.));
    /// ```
    #[cfg(feature = "use_alloc")]
    fn kmerge_by<F>(self, first: F) -> KMergeBy<<Self::Item as IntoIterator>::IntoIter, F>
    where
        Self: Sized,
        Self::Item: IntoIterator,
        F: FnMut(&<Self::Item as IntoIterator>::Item, &<Self::Item as IntoIterator>::Item) -> bool,
    {
        kmerge_by(self, first)
    }

    /// Return an iterator adaptor that iterates over the cartesian product of
    /// the element sets of two iterators `self` and `J`.
    ///
    /// Iterator element type is `(Self::Item, J::Item)`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let it = (0..2).cartesian_product("αβ".chars());
    /// itertools::assert_equal(it, vec![(0, 'α'), (0, 'β'), (1, 'α'), (1, 'β')]);
    /// ```
    fn cartesian_product<J>(self, other: J) -> Product<Self, J::IntoIter>
    where
        Self: Sized,
        Self::Item: Clone,
        J: IntoIterator,
        J::IntoIter: Clone,
    {
        adaptors::cartesian_product(self, other.into_iter())
    }

    /// Return an iterator adaptor that iterates over the cartesian product of
    /// all subiterators returned by meta-iterator `self`.
    ///
    /// All provided iterators must yield the same `Item` type. To generate
    /// the product of iterators yielding multiple types, use the
    /// [`iproduct`] macro instead.
    ///
    /// The iterator element type is `Vec<T>`, where `T` is the iterator element
    /// of the subiterators.
    ///
    /// Note that the iterator is fused.
    ///
    /// ```
    /// use itertools::Itertools;
    /// let mut multi_prod = (0..3).map(|i| (i * 2)..(i * 2 + 2))
    ///     .multi_cartesian_product();
    /// assert_eq!(multi_prod.next(), Some(vec![0, 2, 4]));
    /// assert_eq!(multi_prod.next(), Some(vec![0, 2, 5]));
    /// assert_eq!(multi_prod.next(), Some(vec![0, 3, 4]));
    /// assert_eq!(multi_prod.next(), Some(vec![0, 3, 5]));
    /// assert_eq!(multi_prod.next(), Some(vec![1, 2, 4]));
    /// assert_eq!(multi_prod.next(), Some(vec![1, 2, 5]));
    /// assert_eq!(multi_prod.next(), Some(vec![1, 3, 4]));
    /// assert_eq!(multi_prod.next(), Some(vec![1, 3, 5]));
    /// assert_eq!(multi_prod.next(), None);
    /// ```
    ///
    /// If the adapted iterator is empty, the result is an iterator yielding a single empty vector.
    /// This is known as the [nullary cartesian product](https://en.wikipedia.org/wiki/Empty_product#Nullary_Cartesian_product).
    ///
    /// ```
    /// use itertools::Itertools;
    /// let mut nullary_cartesian_product = (0..0).map(|i| (i * 2)..(i * 2 + 2)).multi_cartesian_product();
    /// assert_eq!(nullary_cartesian_product.next(), Some(vec![]));
    /// assert_eq!(nullary_cartesian_product.next(), None);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn multi_cartesian_product(self) -> MultiProduct<<Self::Item as IntoIterator>::IntoIter>
    where
        Self: Sized,
        Self::Item: IntoIterator,
        <Self::Item as IntoIterator>::IntoIter: Clone,
        <Self::Item as IntoIterator>::Item: Clone,
    {
        adaptors::multi_cartesian_product(self)
    }

    /// Return an iterator adaptor that uses the passed-in closure to
    /// optionally merge together consecutive elements.
    ///
    /// The closure `f` is passed two elements, `previous` and `current` and may
    /// return either (1) `Ok(combined)` to merge the two values or
    /// (2) `Err((previous', current'))` to indicate they can't be merged.
    /// In (2), the value `previous'` is emitted by the iterator.
    /// Either (1) `combined` or (2) `current'` becomes the previous value
    /// when coalesce continues with the next pair of elements to merge. The
    /// value that remains at the end is also emitted by the iterator.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// This iterator is *fused*.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // sum same-sign runs together
    /// let data = vec![-1., -2., -3., 3., 1., 0., -1.];
    /// itertools::assert_equal(data.into_iter().coalesce(|x, y|
    ///         if (x >= 0.) == (y >= 0.) {
    ///             Ok(x + y)
    ///         } else {
    ///             Err((x, y))
    ///         }),
    ///         vec![-6., 4., -1.]);
    /// ```
    fn coalesce<F>(self, f: F) -> Coalesce<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item, Self::Item) -> Result<Self::Item, (Self::Item, Self::Item)>,
    {
        adaptors::coalesce(self, f)
    }

    /// Remove duplicates from sections of consecutive identical elements.
    /// If the iterator is sorted, all elements will be unique.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// This iterator is *fused*.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec![1., 1., 2., 3., 3., 2., 2.];
    /// itertools::assert_equal(data.into_iter().dedup(),
    ///                         vec![1., 2., 3., 2.]);
    /// ```
    fn dedup(self) -> Dedup<Self>
    where
        Self: Sized,
        Self::Item: PartialEq,
    {
        adaptors::dedup(self)
    }

    /// Remove duplicates from sections of consecutive identical elements,
    /// determining equality using a comparison function.
    /// If the iterator is sorted, all elements will be unique.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// This iterator is *fused*.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec![(0, 1.), (1, 1.), (0, 2.), (0, 3.), (1, 3.), (1, 2.), (2, 2.)];
    /// itertools::assert_equal(data.into_iter().dedup_by(|x, y| x.1 == y.1),
    ///                         vec![(0, 1.), (0, 2.), (0, 3.), (1, 2.)]);
    /// ```
    fn dedup_by<Cmp>(self, cmp: Cmp) -> DedupBy<Self, Cmp>
    where
        Self: Sized,
        Cmp: FnMut(&Self::Item, &Self::Item) -> bool,
    {
        adaptors::dedup_by(self, cmp)
    }

    /// Remove duplicates from sections of consecutive identical elements, while keeping a count of
    /// how many repeated elements were present.
    /// If the iterator is sorted, all elements will be unique.
    ///
    /// Iterator element type is `(usize, Self::Item)`.
    ///
    /// This iterator is *fused*.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec!['a', 'a', 'b', 'c', 'c', 'b', 'b'];
    /// itertools::assert_equal(data.into_iter().dedup_with_count(),
    ///                         vec![(2, 'a'), (1, 'b'), (2, 'c'), (2, 'b')]);
    /// ```
    fn dedup_with_count(self) -> DedupWithCount<Self>
    where
        Self: Sized,
    {
        adaptors::dedup_with_count(self)
    }

    /// Remove duplicates from sections of consecutive identical elements, while keeping a count of
    /// how many repeated elements were present.
    /// This will determine equality using a comparison function.
    /// If the iterator is sorted, all elements will be unique.
    ///
    /// Iterator element type is `(usize, Self::Item)`.
    ///
    /// This iterator is *fused*.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec![(0, 'a'), (1, 'a'), (0, 'b'), (0, 'c'), (1, 'c'), (1, 'b'), (2, 'b')];
    /// itertools::assert_equal(data.into_iter().dedup_by_with_count(|x, y| x.1 == y.1),
    ///                         vec![(2, (0, 'a')), (1, (0, 'b')), (2, (0, 'c')), (2, (1, 'b'))]);
    /// ```
    fn dedup_by_with_count<Cmp>(self, cmp: Cmp) -> DedupByWithCount<Self, Cmp>
    where
        Self: Sized,
        Cmp: FnMut(&Self::Item, &Self::Item) -> bool,
    {
        adaptors::dedup_by_with_count(self, cmp)
    }

    /// Return an iterator adaptor that produces elements that appear more than once during the
    /// iteration. Duplicates are detected using hash and equality.
    ///
    /// The iterator is stable, returning the duplicate items in the order in which they occur in
    /// the adapted iterator. Each duplicate item is returned exactly once. If an item appears more
    /// than twice, the second item is the item retained and the rest are discarded.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec![10, 20, 30, 20, 40, 10, 50];
    /// itertools::assert_equal(data.into_iter().duplicates(),
    ///                         vec![20, 10]);
    /// ```
    #[cfg(feature = "use_std")]
    fn duplicates(self) -> Duplicates<Self>
    where
        Self: Sized,
        Self::Item: Eq + Hash,
    {
        duplicates_impl::duplicates(self)
    }

    /// Return an iterator adaptor that produces elements that appear more than once during the
    /// iteration. Duplicates are detected using hash and equality.
    ///
    /// Duplicates are detected by comparing the key they map to with the keying function `f` by
    /// hash and equality. The keys are stored in a hash map in the iterator.
    ///
    /// The iterator is stable, returning the duplicate items in the order in which they occur in
    /// the adapted iterator. Each duplicate item is returned exactly once. If an item appears more
    /// than twice, the second item is the item retained and the rest are discarded.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec!["a", "bb", "aa", "c", "ccc"];
    /// itertools::assert_equal(data.into_iter().duplicates_by(|s| s.len()),
    ///                         vec!["aa", "c"]);
    /// ```
    #[cfg(feature = "use_std")]
    fn duplicates_by<V, F>(self, f: F) -> DuplicatesBy<Self, V, F>
    where
        Self: Sized,
        V: Eq + Hash,
        F: FnMut(&Self::Item) -> V,
    {
        duplicates_impl::duplicates_by(self, f)
    }

    /// Return an iterator adaptor that filters out elements that have
    /// already been produced once during the iteration. Duplicates
    /// are detected using hash and equality.
    ///
    /// Clones of visited elements are stored in a hash set in the
    /// iterator.
    ///
    /// The iterator is stable, returning the non-duplicate items in the order
    /// in which they occur in the adapted iterator. In a set of duplicate
    /// items, the first item encountered is the item retained.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec![10, 20, 30, 20, 40, 10, 50];
    /// itertools::assert_equal(data.into_iter().unique(),
    ///                         vec![10, 20, 30, 40, 50]);
    /// ```
    #[cfg(feature = "use_std")]
    fn unique(self) -> Unique<Self>
    where
        Self: Sized,
        Self::Item: Clone + Eq + Hash,
    {
        unique_impl::unique(self)
    }

    /// Return an iterator adaptor that filters out elements that have
    /// already been produced once during the iteration.
    ///
    /// Duplicates are detected by comparing the key they map to
    /// with the keying function `f` by hash and equality.
    /// The keys are stored in a hash set in the iterator.
    ///
    /// The iterator is stable, returning the non-duplicate items in the order
    /// in which they occur in the adapted iterator. In a set of duplicate
    /// items, the first item encountered is the item retained.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec!["a", "bb", "aa", "c", "ccc"];
    /// itertools::assert_equal(data.into_iter().unique_by(|s| s.len()),
    ///                         vec!["a", "bb", "ccc"]);
    /// ```
    #[cfg(feature = "use_std")]
    fn unique_by<V, F>(self, f: F) -> UniqueBy<Self, V, F>
    where
        Self: Sized,
        V: Eq + Hash,
        F: FnMut(&Self::Item) -> V,
    {
        unique_impl::unique_by(self, f)
    }

    /// Return an iterator adaptor that borrows from this iterator and
    /// takes items while the closure `accept` returns `true`.
    ///
    /// This adaptor can only be used on iterators that implement `PeekingNext`
    /// like `.peekable()`, `put_back` and a few other collection iterators.
    ///
    /// The last and rejected element (first `false`) is still available when
    /// `peeking_take_while` is done.
    ///
    ///
    /// See also [`.take_while_ref()`](Itertools::take_while_ref)
    /// which is a similar adaptor.
    fn peeking_take_while<F>(&mut self, accept: F) -> PeekingTakeWhile<Self, F>
    where
        Self: Sized + PeekingNext,
        F: FnMut(&Self::Item) -> bool,
    {
        peeking_take_while::peeking_take_while(self, accept)
    }

    /// Return an iterator adaptor that borrows from a `Clone`-able iterator
    /// to only pick off elements while the predicate `accept` returns `true`.
    ///
    /// It uses the `Clone` trait to restore the original iterator so that the
    /// last and rejected element (first `false`) is still available when
    /// `take_while_ref` is done.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let mut hexadecimals = "0123456789abcdef".chars();
    ///
    /// let decimals = hexadecimals.take_while_ref(|c| c.is_numeric())
    ///                            .collect::<String>();
    /// assert_eq!(decimals, "0123456789");
    /// assert_eq!(hexadecimals.next(), Some('a'));
    ///
    /// ```
    fn take_while_ref<F>(&mut self, accept: F) -> TakeWhileRef<Self, F>
    where
        Self: Clone,
        F: FnMut(&Self::Item) -> bool,
    {
        adaptors::take_while_ref(self, accept)
    }

    /// Returns an iterator adaptor that consumes elements while the given
    /// predicate is `true`, *including* the element for which the predicate
    /// first returned `false`.
    ///
    /// The [`.take_while()`][std::iter::Iterator::take_while] adaptor is useful
    /// when you want items satisfying a predicate, but to know when to stop
    /// taking elements, we have to consume that first element that doesn't
    /// satisfy the predicate. This adaptor includes that element where
    /// [`.take_while()`][std::iter::Iterator::take_while] would drop it.
    ///
    /// The [`.take_while_ref()`][crate::Itertools::take_while_ref] adaptor
    /// serves a similar purpose, but this adaptor doesn't require [`Clone`]ing
    /// the underlying elements.
    ///
    /// ```rust
    /// # use itertools::Itertools;
    /// let items = vec![1, 2, 3, 4, 5];
    /// let filtered: Vec<_> = items
    ///     .into_iter()
    ///     .take_while_inclusive(|&n| n % 3 != 0)
    ///     .collect();
    ///
    /// assert_eq!(filtered, vec![1, 2, 3]);
    /// ```
    ///
    /// ```rust
    /// # use itertools::Itertools;
    /// let items = vec![1, 2, 3, 4, 5];
    ///
    /// let take_while_inclusive_result: Vec<_> = items
    ///     .iter()
    ///     .copied()
    ///     .take_while_inclusive(|&n| n % 3 != 0)
    ///     .collect();
    /// let take_while_result: Vec<_> = items
    ///     .into_iter()
    ///     .take_while(|&n| n % 3 != 0)
    ///     .collect();
    ///
    /// assert_eq!(take_while_inclusive_result, vec![1, 2, 3]);
    /// assert_eq!(take_while_result, vec![1, 2]);
    /// // both iterators have the same items remaining at this point---the 3
    /// // is lost from the `take_while` vec
    /// ```
    ///
    /// ```rust
    /// # use itertools::Itertools;
    /// #[derive(Debug, PartialEq)]
    /// struct NoCloneImpl(i32);
    ///
    /// let non_clonable_items: Vec<_> = vec![1, 2, 3, 4, 5]
    ///     .into_iter()
    ///     .map(NoCloneImpl)
    ///     .collect();
    /// let filtered: Vec<_> = non_clonable_items
    ///     .into_iter()
    ///     .take_while_inclusive(|n| n.0 % 3 != 0)
    ///     .collect();
    /// let expected: Vec<_> = vec![1, 2, 3].into_iter().map(NoCloneImpl).collect();
    /// assert_eq!(filtered, expected);
    #[doc(alias = "take_until")]
    fn take_while_inclusive<F>(self, accept: F) -> TakeWhileInclusive<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> bool,
    {
        take_while_inclusive::TakeWhileInclusive::new(self, accept)
    }

    /// Return an iterator adaptor that filters `Option<A>` iterator elements
    /// and produces `A`. Stops on the first `None` encountered.
    ///
    /// Iterator element type is `A`, the unwrapped element.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // List all hexadecimal digits
    /// itertools::assert_equal(
    ///     (0..).map(|i| std::char::from_digit(i, 16)).while_some(),
    ///     "0123456789abcdef".chars());
    ///
    /// ```
    fn while_some<A>(self) -> WhileSome<Self>
    where
        Self: Sized + Iterator<Item = Option<A>>,
    {
        adaptors::while_some(self)
    }

    /// Return an iterator adaptor that iterates over the combinations of the
    /// elements from an iterator.
    ///
    /// Iterator element can be any homogeneous tuple of type `Self::Item` with
    /// size up to 12.
    ///
    /// # Guarantees
    ///
    /// If the adapted iterator is deterministic,
    /// this iterator adapter yields items in a reliable order.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let mut v = Vec::new();
    /// for (a, b) in (1..5).tuple_combinations() {
    ///     v.push((a, b));
    /// }
    /// assert_eq!(v, vec![(1, 2), (1, 3), (1, 4), (2, 3), (2, 4), (3, 4)]);
    ///
    /// let mut it = (1..5).tuple_combinations();
    /// assert_eq!(Some((1, 2, 3)), it.next());
    /// assert_eq!(Some((1, 2, 4)), it.next());
    /// assert_eq!(Some((1, 3, 4)), it.next());
    /// assert_eq!(Some((2, 3, 4)), it.next());
    /// assert_eq!(None, it.next());
    ///
    /// // this requires a type hint
    /// let it = (1..5).tuple_combinations::<(_, _, _)>();
    /// itertools::assert_equal(it, vec![(1, 2, 3), (1, 2, 4), (1, 3, 4), (2, 3, 4)]);
    ///
    /// // you can also specify the complete type
    /// use itertools::TupleCombinations;
    /// use std::ops::Range;
    ///
    /// let it: TupleCombinations<Range<u32>, (u32, u32, u32)> = (1..5).tuple_combinations();
    /// itertools::assert_equal(it, vec![(1, 2, 3), (1, 2, 4), (1, 3, 4), (2, 3, 4)]);
    /// ```
    fn tuple_combinations<T>(self) -> TupleCombinations<Self, T>
    where
        Self: Sized + Clone,
        Self::Item: Clone,
        T: adaptors::HasCombination<Self>,
    {
        adaptors::tuple_combinations(self)
    }

    /// Return an iterator adaptor that iterates over the combinations of the
    /// elements from an iterator.
    ///
    /// Iterator element type is [Self::Item; K]. The iterator produces a new
    /// array per iteration, and clones the iterator elements.
    ///
    /// # Guarantees
    ///
    /// If the adapted iterator is deterministic,
    /// this iterator adapter yields items in a reliable order.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let mut v = Vec::new();
    /// for [a, b] in (1..5).array_combinations() {
    ///     v.push([a, b]);
    /// }
    /// assert_eq!(v, vec![[1, 2], [1, 3], [1, 4], [2, 3], [2, 4], [3, 4]]);
    ///
    /// let mut it = (1..5).array_combinations();
    /// assert_eq!(Some([1, 2, 3]), it.next());
    /// assert_eq!(Some([1, 2, 4]), it.next());
    /// assert_eq!(Some([1, 3, 4]), it.next());
    /// assert_eq!(Some([2, 3, 4]), it.next());
    /// assert_eq!(None, it.next());
    ///
    /// // this requires a type hint
    /// let it = (1..5).array_combinations::<3>();
    /// itertools::assert_equal(it, vec![[1, 2, 3], [1, 2, 4], [1, 3, 4], [2, 3, 4]]);
    ///
    /// // you can also specify the complete type
    /// use itertools::ArrayCombinations;
    /// use std::ops::Range;
    ///
    /// let it: ArrayCombinations<Range<u32>, 3> = (1..5).array_combinations();
    /// itertools::assert_equal(it, vec![[1, 2, 3], [1, 2, 4], [1, 3, 4], [2, 3, 4]]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn array_combinations<const K: usize>(self) -> ArrayCombinations<Self, K>
    where
        Self: Sized + Clone,
        Self::Item: Clone,
    {
        combinations::array_combinations(self)
    }

    /// Return an iterator adaptor that iterates over the `k`-length combinations of
    /// the elements from an iterator.
    ///
    /// Iterator element type is `Vec<Self::Item>`. The iterator produces a new `Vec` per iteration,
    /// and clones the iterator elements.
    ///
    /// # Guarantees
    ///
    /// If the adapted iterator is deterministic,
    /// this iterator adapter yields items in a reliable order.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let it = (1..5).combinations(3);
    /// itertools::assert_equal(it, vec![
    ///     vec![1, 2, 3],
    ///     vec![1, 2, 4],
    ///     vec![1, 3, 4],
    ///     vec![2, 3, 4],
    /// ]);
    /// ```
    ///
    /// Note: Combinations does not take into account the equality of the iterated values.
    /// ```
    /// use itertools::Itertools;
    ///
    /// let it = vec![1, 2, 2].into_iter().combinations(2);
    /// itertools::assert_equal(it, vec![
    ///     vec![1, 2], // Note: these are the same
    ///     vec![1, 2], // Note: these are the same
    ///     vec![2, 2],
    /// ]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn combinations(self, k: usize) -> Combinations<Self>
    where
        Self: Sized,
        Self::Item: Clone,
    {
        combinations::combinations(self, k)
    }

    /// Return an iterator that iterates over the `k`-length combinations of
    /// the elements from an iterator, with replacement.
    ///
    /// Iterator element type is `Vec<Self::Item>`. The iterator produces a new `Vec` per iteration,
    /// and clones the iterator elements.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let it = (1..4).combinations_with_replacement(2);
    /// itertools::assert_equal(it, vec![
    ///     vec![1, 1],
    ///     vec![1, 2],
    ///     vec![1, 3],
    ///     vec![2, 2],
    ///     vec![2, 3],
    ///     vec![3, 3],
    /// ]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn combinations_with_replacement(self, k: usize) -> CombinationsWithReplacement<Self>
    where
        Self: Sized,
        Self::Item: Clone,
    {
        combinations_with_replacement::combinations_with_replacement(self, k)
    }

    /// Return an iterator adaptor that iterates over all k-permutations of the
    /// elements from an iterator.
    ///
    /// Iterator element type is `Vec<Self::Item>` with length `k`. The iterator
    /// produces a new `Vec` per iteration, and clones the iterator elements.
    ///
    /// If `k` is greater than the length of the input iterator, the resultant
    /// iterator adaptor will be empty.
    ///
    /// If you are looking for permutations with replacements,
    /// use `repeat_n(iter, k).multi_cartesian_product()` instead.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let perms = (5..8).permutations(2);
    /// itertools::assert_equal(perms, vec![
    ///     vec![5, 6],
    ///     vec![5, 7],
    ///     vec![6, 5],
    ///     vec![6, 7],
    ///     vec![7, 5],
    ///     vec![7, 6],
    /// ]);
    /// ```
    ///
    /// Note: Permutations does not take into account the equality of the iterated values.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let it = vec![2, 2].into_iter().permutations(2);
    /// itertools::assert_equal(it, vec![
    ///     vec![2, 2], // Note: these are the same
    ///     vec![2, 2], // Note: these are the same
    /// ]);
    /// ```
    ///
    /// Note: The source iterator is collected lazily, and will not be
    /// re-iterated if the permutations adaptor is completed and re-iterated.
    #[cfg(feature = "use_alloc")]
    fn permutations(self, k: usize) -> Permutations<Self>
    where
        Self: Sized,
        Self::Item: Clone,
    {
        permutations::permutations(self, k)
    }

    /// Return an iterator that iterates through the powerset of the elements from an
    /// iterator.
    ///
    /// Iterator element type is `Vec<Self::Item>`. The iterator produces a new `Vec`
    /// per iteration, and clones the iterator elements.
    ///
    /// The powerset of a set contains all subsets including the empty set and the full
    /// input set. A powerset has length _2^n_ where _n_ is the length of the input
    /// set.
    ///
    /// Each `Vec` produced by this iterator represents a subset of the elements
    /// produced by the source iterator.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let sets = (1..4).powerset().collect::<Vec<_>>();
    /// itertools::assert_equal(sets, vec![
    ///     vec![],
    ///     vec![1],
    ///     vec![2],
    ///     vec![3],
    ///     vec![1, 2],
    ///     vec![1, 3],
    ///     vec![2, 3],
    ///     vec![1, 2, 3],
    /// ]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn powerset(self) -> Powerset<Self>
    where
        Self: Sized,
        Self::Item: Clone,
    {
        powerset::powerset(self)
    }

    /// Return an iterator adaptor that pads the sequence to a minimum length of
    /// `min` by filling missing elements using a closure `f`.
    ///
    /// Iterator element type is `Self::Item`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let it = (0..5).pad_using(10, |i| 2*i);
    /// itertools::assert_equal(it, vec![0, 1, 2, 3, 4, 10, 12, 14, 16, 18]);
    ///
    /// let it = (0..10).pad_using(5, |i| 2*i);
    /// itertools::assert_equal(it, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    ///
    /// let it = (0..5).pad_using(10, |i| 2*i).rev();
    /// itertools::assert_equal(it, vec![18, 16, 14, 12, 10, 4, 3, 2, 1, 0]);
    /// ```
    fn pad_using<F>(self, min: usize, f: F) -> PadUsing<Self, F>
    where
        Self: Sized,
        F: FnMut(usize) -> Self::Item,
    {
        pad_tail::pad_using(self, min, f)
    }

    /// Return an iterator adaptor that combines each element with a `Position` to
    /// ease special-case handling of the first or last elements.
    ///
    /// Iterator element type is
    /// [`(Position, Self::Item)`](Position)
    ///
    /// ```
    /// use itertools::{Itertools, Position};
    ///
    /// let it = (0..4).with_position();
    /// itertools::assert_equal(it,
    ///                         vec![(Position::First, 0),
    ///                              (Position::Middle, 1),
    ///                              (Position::Middle, 2),
    ///                              (Position::Last, 3)]);
    ///
    /// let it = (0..1).with_position();
    /// itertools::assert_equal(it, vec![(Position::Only, 0)]);
    /// ```
    fn with_position(self) -> WithPosition<Self>
    where
        Self: Sized,
    {
        with_position::with_position(self)
    }

    /// Return an iterator adaptor that yields the indices of all elements
    /// satisfying a predicate, counted from the start of the iterator.
    ///
    /// Equivalent to `iter.enumerate().filter(|(_, v)| predicate(*v)).map(|(i, _)| i)`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec![1, 2, 3, 3, 4, 6, 7, 9];
    /// itertools::assert_equal(data.iter().positions(|v| v % 2 == 0), vec![1, 4, 5]);
    ///
    /// itertools::assert_equal(data.iter().positions(|v| v % 2 == 1).rev(), vec![7, 6, 3, 2, 0]);
    /// ```
    fn positions<P>(self, predicate: P) -> Positions<Self, P>
    where
        Self: Sized,
        P: FnMut(Self::Item) -> bool,
    {
        adaptors::positions(self, predicate)
    }

    /// Return an iterator adaptor that applies a mutating function
    /// to each element before yielding it.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let input = vec![vec![1], vec![3, 2, 1]];
    /// let it = input.into_iter().update(|v| v.push(0));
    /// itertools::assert_equal(it, vec![vec![1, 0], vec![3, 2, 1, 0]]);
    /// ```
    fn update<F>(self, updater: F) -> Update<Self, F>
    where
        Self: Sized,
        F: FnMut(&mut Self::Item),
    {
        adaptors::update(self, updater)
    }

    // non-adaptor methods
    /// Advances the iterator and returns the next items grouped in an array of
    /// a specific size.
    ///
    /// If there are enough elements to be grouped in an array, then the array
    /// is returned inside `Some`, otherwise `None` is returned.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let mut iter = 1..5;
    ///
    /// assert_eq!(Some([1, 2]), iter.next_array());
    /// ```
    fn next_array<const N: usize>(&mut self) -> Option<[Self::Item; N]>
    where
        Self: Sized,
    {
        next_array::next_array(self)
    }

    /// Collects all items from the iterator into an array of a specific size.
    ///
    /// If the number of elements inside the iterator is **exactly** equal to
    /// the array size, then the array is returned inside `Some`, otherwise
    /// `None` is returned.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let iter = 1..3;
    ///
    /// if let Some([x, y]) = iter.collect_array() {
    ///     assert_eq!([x, y], [1, 2])
    /// } else {
    ///     panic!("Expected two elements")
    /// }
    /// ```
    fn collect_array<const N: usize>(mut self) -> Option<[Self::Item; N]>
    where
        Self: Sized,
    {
        self.next_array().filter(|_| self.next().is_none())
    }

    /// Advances the iterator and returns the next items grouped in a tuple of
    /// a specific size (up to 12).
    ///
    /// If there are enough elements to be grouped in a tuple, then the tuple is
    /// returned inside `Some`, otherwise `None` is returned.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let mut iter = 1..5;
    ///
    /// assert_eq!(Some((1, 2)), iter.next_tuple());
    /// ```
    fn next_tuple<T>(&mut self) -> Option<T>
    where
        Self: Sized + Iterator<Item = T::Item>,
        T: traits::HomogeneousTuple,
    {
        T::collect_from_iter_no_buf(self)
    }

    /// Collects all items from the iterator into a tuple of a specific size
    /// (up to 12).
    ///
    /// If the number of elements inside the iterator is **exactly** equal to
    /// the tuple size, then the tuple is returned inside `Some`, otherwise
    /// `None` is returned.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let iter = 1..3;
    ///
    /// if let Some((x, y)) = iter.collect_tuple() {
    ///     assert_eq!((x, y), (1, 2))
    /// } else {
    ///     panic!("Expected two elements")
    /// }
    /// ```
    fn collect_tuple<T>(mut self) -> Option<T>
    where
        Self: Sized + Iterator<Item = T::Item>,
        T: traits::HomogeneousTuple,
    {
        match self.next_tuple() {
            elt @ Some(_) => match self.next() {
                Some(_) => None,
                None => elt,
            },
            _ => None,
        }
    }

    /// Find the position and value of the first element satisfying a predicate.
    ///
    /// The iterator is not advanced past the first element found.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let text = "Hα";
    /// assert_eq!(text.chars().find_position(|ch| ch.is_lowercase()), Some((1, 'α')));
    /// ```
    fn find_position<P>(&mut self, mut pred: P) -> Option<(usize, Self::Item)>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        self.enumerate().find(|(_, elt)| pred(elt))
    }
    /// Find the value of the first element satisfying a predicate or return the last element, if any.
    ///
    /// The iterator is not advanced past the first element found.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let numbers = [1, 2, 3, 4];
    /// assert_eq!(numbers.iter().find_or_last(|&&x| x > 5), Some(&4));
    /// assert_eq!(numbers.iter().find_or_last(|&&x| x > 2), Some(&3));
    /// assert_eq!(std::iter::empty::<i32>().find_or_last(|&x| x > 5), None);
    ///
    /// // An iterator of Results can return the first Ok or the last Err:
    /// let input = vec![Err(()), Ok(11), Err(()), Ok(22)];
    /// assert_eq!(input.into_iter().find_or_last(Result::is_ok), Some(Ok(11)));
    ///
    /// let input: Vec<Result<(), i32>> = vec![Err(11), Err(22)];
    /// assert_eq!(input.into_iter().find_or_last(Result::is_ok), Some(Err(22)));
    ///
    /// assert_eq!(std::iter::empty::<Result<(), i32>>().find_or_last(Result::is_ok), None);
    /// ```
    fn find_or_last<P>(mut self, mut predicate: P) -> Option<Self::Item>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        let mut prev = None;
        self.find_map(|x| {
            if predicate(&x) {
                Some(x)
            } else {
                prev = Some(x);
                None
            }
        })
        .or(prev)
    }
    /// Find the value of the first element satisfying a predicate or return the first element, if any.
    ///
    /// The iterator is not advanced past the first element found.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let numbers = [1, 2, 3, 4];
    /// assert_eq!(numbers.iter().find_or_first(|&&x| x > 5), Some(&1));
    /// assert_eq!(numbers.iter().find_or_first(|&&x| x > 2), Some(&3));
    /// assert_eq!(std::iter::empty::<i32>().find_or_first(|&x| x > 5), None);
    ///
    /// // An iterator of Results can return the first Ok or the first Err:
    /// let input = vec![Err(()), Ok(11), Err(()), Ok(22)];
    /// assert_eq!(input.into_iter().find_or_first(Result::is_ok), Some(Ok(11)));
    ///
    /// let input: Vec<Result<(), i32>> = vec![Err(11), Err(22)];
    /// assert_eq!(input.into_iter().find_or_first(Result::is_ok), Some(Err(11)));
    ///
    /// assert_eq!(std::iter::empty::<Result<(), i32>>().find_or_first(Result::is_ok), None);
    /// ```
    fn find_or_first<P>(mut self, mut predicate: P) -> Option<Self::Item>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        let first = self.next()?;
        Some(if predicate(&first) {
            first
        } else {
            self.find(|x| predicate(x)).unwrap_or(first)
        })
    }
    /// Returns `true` if the given item is present in this iterator.
    ///
    /// This method is short-circuiting. If the given item is present in this
    /// iterator, this method will consume the iterator up-to-and-including
    /// the item. If the given item is not present in this iterator, the
    /// iterator will be exhausted.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// #[derive(PartialEq, Debug)]
    /// enum Enum { A, B, C, D, E, }
    ///
    /// let mut iter = vec![Enum::A, Enum::B, Enum::C, Enum::D].into_iter();
    ///
    /// // search `iter` for `B`
    /// assert_eq!(iter.contains(&Enum::B), true);
    /// // `B` was found, so the iterator now rests at the item after `B` (i.e, `C`).
    /// assert_eq!(iter.next(), Some(Enum::C));
    ///
    /// // search `iter` for `E`
    /// assert_eq!(iter.contains(&Enum::E), false);
    /// // `E` wasn't found, so `iter` is now exhausted
    /// assert_eq!(iter.next(), None);
    /// ```
    fn contains<Q>(&mut self, query: &Q) -> bool
    where
        Self: Sized,
        Self::Item: Borrow<Q>,
        Q: PartialEq + ?Sized,
    {
        self.any(|x| x.borrow() == query)
    }

    /// Check whether all elements compare equal.
    ///
    /// Empty iterators are considered to have equal elements:
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec![1, 1, 1, 2, 2, 3, 3, 3, 4, 5, 5];
    /// assert!(!data.iter().all_equal());
    /// assert!(data[0..3].iter().all_equal());
    /// assert!(data[3..5].iter().all_equal());
    /// assert!(data[5..8].iter().all_equal());
    ///
    /// let data : Option<usize> = None;
    /// assert!(data.into_iter().all_equal());
    /// ```
    fn all_equal(&mut self) -> bool
    where
        Self: Sized,
        Self::Item: PartialEq,
    {
        match self.next() {
            None => true,
            Some(a) => self.all(|x| a == x),
        }
    }

    /// If there are elements and they are all equal, return a single copy of that element.
    /// If there are no elements, return an Error containing None.
    /// If there are elements and they are not all equal, return a tuple containing the first
    /// two non-equal elements found.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec![1, 1, 1, 2, 2, 3, 3, 3, 4, 5, 5];
    /// assert_eq!(data.iter().all_equal_value(), Err(Some((&1, &2))));
    /// assert_eq!(data[0..3].iter().all_equal_value(), Ok(&1));
    /// assert_eq!(data[3..5].iter().all_equal_value(), Ok(&2));
    /// assert_eq!(data[5..8].iter().all_equal_value(), Ok(&3));
    ///
    /// let data : Option<usize> = None;
    /// assert_eq!(data.into_iter().all_equal_value(), Err(None));
    /// ```
    #[allow(clippy::type_complexity)]
    fn all_equal_value(&mut self) -> Result<Self::Item, Option<(Self::Item, Self::Item)>>
    where
        Self: Sized,
        Self::Item: PartialEq,
    {
        let first = self.next().ok_or(None)?;
        let other = self.find(|x| x != &first);
        if let Some(other) = other {
            Err(Some((first, other)))
        } else {
            Ok(first)
        }
    }

    /// Check whether all elements are unique (non equal).
    ///
    /// Empty iterators are considered to have unique elements:
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec![1, 2, 3, 4, 1, 5];
    /// assert!(!data.iter().all_unique());
    /// assert!(data[0..4].iter().all_unique());
    /// assert!(data[1..6].iter().all_unique());
    ///
    /// let data : Option<usize> = None;
    /// assert!(data.into_iter().all_unique());
    /// ```
    #[cfg(feature = "use_std")]
    fn all_unique(&mut self) -> bool
    where
        Self: Sized,
        Self::Item: Eq + Hash,
    {
        let mut used = HashSet::new();
        self.all(move |elt| used.insert(elt))
    }

    /// Consume the first `n` elements from the iterator eagerly,
    /// and return the same iterator again.
    ///
    /// It works similarly to `.skip(n)` except it is eager and
    /// preserves the iterator type.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let iter = "αβγ".chars().dropping(2);
    /// itertools::assert_equal(iter, "γ".chars());
    /// ```
    ///
    /// *Fusing notes: if the iterator is exhausted by dropping,
    /// the result of calling `.next()` again depends on the iterator implementation.*
    fn dropping(mut self, n: usize) -> Self
    where
        Self: Sized,
    {
        if n > 0 {
            self.nth(n - 1);
        }
        self
    }

    /// Consume the last `n` elements from the iterator eagerly,
    /// and return the same iterator again.
    ///
    /// This is only possible on double ended iterators. `n` may be
    /// larger than the number of elements.
    ///
    /// Note: This method is eager, dropping the back elements immediately and
    /// preserves the iterator type.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let init = vec![0, 3, 6, 9].into_iter().dropping_back(1);
    /// itertools::assert_equal(init, vec![0, 3, 6]);
    /// ```
    fn dropping_back(mut self, n: usize) -> Self
    where
        Self: Sized + DoubleEndedIterator,
    {
        if n > 0 {
            (&mut self).rev().nth(n - 1);
        }
        self
    }

    /// Combine all an iterator's elements into one element by using [`Extend`].
    ///
    /// This combinator will extend the first item with each of the rest of the
    /// items of the iterator. If the iterator is empty, the default value of
    /// `I::Item` is returned.
    ///
    /// ```rust
    /// use itertools::Itertools;
    ///
    /// let input = vec![vec![1], vec![2, 3], vec![4, 5, 6]];
    /// assert_eq!(input.into_iter().concat(),
    ///            vec![1, 2, 3, 4, 5, 6]);
    /// ```
    fn concat(self) -> Self::Item
    where
        Self: Sized,
        Self::Item:
            Extend<<<Self as Iterator>::Item as IntoIterator>::Item> + IntoIterator + Default,
    {
        concat(self)
    }

    /// `.collect_vec()` is simply a type specialization of [`Iterator::collect`],
    /// for convenience.
    #[cfg(feature = "use_alloc")]
    fn collect_vec(self) -> Vec<Self::Item>
    where
        Self: Sized,
    {
        self.collect()
    }

    /// `.try_collect()` is more convenient way of writing
    /// `.collect::<Result<_, _>>()`
    ///
    /// # Example
    ///
    /// ```
    /// use std::{fs, io};
    /// use itertools::Itertools;
    ///
    /// fn process_dir_entries(entries: &[fs::DirEntry]) {
    ///     // ...
    ///     # let _ = entries;
    /// }
    ///
    /// fn do_stuff() -> io::Result<()> {
    ///     let entries: Vec<_> = fs::read_dir(".")?.try_collect()?;
    ///     process_dir_entries(&entries);
    ///
    ///     Ok(())
    /// }
    ///
    /// # let _ = do_stuff;
    /// ```
    fn try_collect<T, U, E>(self) -> Result<U, E>
    where
        Self: Sized + Iterator<Item = Result<T, E>>,
        Result<U, E>: FromIterator<Result<T, E>>,
    {
        self.collect()
    }

    /// Assign to each reference in `self` from the `from` iterator,
    /// stopping at the shortest of the two iterators.
    ///
    /// The `from` iterator is queried for its next element before the `self`
    /// iterator, and if either is exhausted the method is done.
    ///
    /// Return the number of elements written.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let mut xs = [0; 4];
    /// xs.iter_mut().set_from(1..);
    /// assert_eq!(xs, [1, 2, 3, 4]);
    /// ```
    #[inline]
    fn set_from<'a, A: 'a, J>(&mut self, from: J) -> usize
    where
        Self: Iterator<Item = &'a mut A>,
        J: IntoIterator<Item = A>,
    {
        from.into_iter()
            .zip(self)
            .map(|(new, old)| *old = new)
            .count()
    }

    /// Combine all iterator elements into one `String`, separated by `sep`.
    ///
    /// Use the `Display` implementation of each element.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// assert_eq!(["a", "b", "c"].iter().join(", "), "a, b, c");
    /// assert_eq!([1, 2, 3].iter().join(", "), "1, 2, 3");
    /// ```
    #[cfg(feature = "use_alloc")]
    fn join(&mut self, sep: &str) -> String
    where
        Self::Item: std::fmt::Display,
    {
        match self.next() {
            None => String::new(),
            Some(first_elt) => {
                // estimate lower bound of capacity needed
                let (lower, _) = self.size_hint();
                let mut result = String::with_capacity(sep.len() * lower);
                write!(&mut result, "{}", first_elt).unwrap();
                self.for_each(|elt| {
                    result.push_str(sep);
                    write!(&mut result, "{}", elt).unwrap();
                });
                result
            }
        }
    }

    /// Format all iterator elements, separated by `sep`.
    ///
    /// All elements are formatted (any formatting trait)
    /// with `sep` inserted between each element.
    ///
    /// **Panics** if the formatter helper is formatted more than once.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = [1.1, 2.71828, -3.];
    /// assert_eq!(
    ///     format!("{:.2}", data.iter().format(", ")),
    ///            "1.10, 2.72, -3.00");
    /// ```
    fn format(self, sep: &str) -> Format<Self>
    where
        Self: Sized,
    {
        format::new_format_default(self, sep)
    }

    /// Format all iterator elements, separated by `sep`.
    ///
    /// This is a customizable version of [`.format()`](Itertools::format).
    ///
    /// The supplied closure `format` is called once per iterator element,
    /// with two arguments: the element and a callback that takes a
    /// `&Display` value, i.e. any reference to type that implements `Display`.
    ///
    /// Using `&format_args!(...)` is the most versatile way to apply custom
    /// element formatting. The callback can be called multiple times if needed.
    ///
    /// **Panics** if the formatter helper is formatted more than once.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = [1.1, 2.71828, -3.];
    /// let data_formatter = data.iter().format_with(", ", |elt, f| f(&format_args!("{:.2}", elt)));
    /// assert_eq!(format!("{}", data_formatter),
    ///            "1.10, 2.72, -3.00");
    ///
    /// // .format_with() is recursively composable
    /// let matrix = [[1., 2., 3.],
    ///               [4., 5., 6.]];
    /// let matrix_formatter = matrix.iter().format_with("\n", |row, f| {
    ///                                 f(&row.iter().format_with(", ", |elt, g| g(&elt)))
    ///                              });
    /// assert_eq!(format!("{}", matrix_formatter),
    ///            "1, 2, 3\n4, 5, 6");
    ///
    ///
    /// ```
    fn format_with<F>(self, sep: &str, format: F) -> FormatWith<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item, &mut dyn FnMut(&dyn fmt::Display) -> fmt::Result) -> fmt::Result,
    {
        format::new_format(self, sep, format)
    }

    /// Fold `Result` values from an iterator.
    ///
    /// Only `Ok` values are folded. If no error is encountered, the folded
    /// value is returned inside `Ok`. Otherwise, the operation terminates
    /// and returns the first `Err` value it encounters. No iterator elements are
    /// consumed after the first error.
    ///
    /// The first accumulator value is the `start` parameter.
    /// Each iteration passes the accumulator value and the next value inside `Ok`
    /// to the fold function `f` and its return value becomes the new accumulator value.
    ///
    /// For example the sequence *Ok(1), Ok(2), Ok(3)* will result in a
    /// computation like this:
    ///
    /// ```no_run
    /// # let start = 0;
    /// # let f = |x, y| x + y;
    /// let mut accum = start;
    /// accum = f(accum, 1);
    /// accum = f(accum, 2);
    /// accum = f(accum, 3);
    /// # let _ = accum;
    /// ```
    ///
    /// With a `start` value of 0 and an addition as folding function,
    /// this effectively results in *((0 + 1) + 2) + 3*
    ///
    /// ```
    /// use std::ops::Add;
    /// use itertools::Itertools;
    ///
    /// let values = [1, 2, -2, -1, 2, 1];
    /// assert_eq!(
    ///     values.iter()
    ///           .map(Ok::<_, ()>)
    ///           .fold_ok(0, Add::add),
    ///     Ok(3)
    /// );
    /// assert!(
    ///     values.iter()
    ///           .map(|&x| if x >= 0 { Ok(x) } else { Err("Negative number") })
    ///           .fold_ok(0, Add::add)
    ///           .is_err()
    /// );
    /// ```
    fn fold_ok<A, E, B, F>(&mut self, mut start: B, mut f: F) -> Result<B, E>
    where
        Self: Iterator<Item = Result<A, E>>,
        F: FnMut(B, A) -> B,
    {
        for elt in self {
            match elt {
                Ok(v) => start = f(start, v),
                Err(u) => return Err(u),
            }
        }
        Ok(start)
    }

    /// Fold `Option` values from an iterator.
    ///
    /// Only `Some` values are folded. If no `None` is encountered, the folded
    /// value is returned inside `Some`. Otherwise, the operation terminates
    /// and returns `None`. No iterator elements are consumed after the `None`.
    ///
    /// This is the `Option` equivalent to [`fold_ok`](Itertools::fold_ok).
    ///
    /// ```
    /// use std::ops::Add;
    /// use itertools::Itertools;
    ///
    /// let mut values = vec![Some(1), Some(2), Some(-2)].into_iter();
    /// assert_eq!(values.fold_options(5, Add::add), Some(5 + 1 + 2 - 2));
    ///
    /// let mut more_values = vec![Some(2), None, Some(0)].into_iter();
    /// assert!(more_values.fold_options(0, Add::add).is_none());
    /// assert_eq!(more_values.next().unwrap(), Some(0));
    /// ```
    fn fold_options<A, B, F>(&mut self, mut start: B, mut f: F) -> Option<B>
    where
        Self: Iterator<Item = Option<A>>,
        F: FnMut(B, A) -> B,
    {
        for elt in self {
            match elt {
                Some(v) => start = f(start, v),
                None => return None,
            }
        }
        Some(start)
    }

    /// Accumulator of the elements in the iterator.
    ///
    /// Like `.fold()`, without a base case. If the iterator is
    /// empty, return `None`. With just one element, return it.
    /// Otherwise elements are accumulated in sequence using the closure `f`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// assert_eq!((0..10).fold1(|x, y| x + y).unwrap_or(0), 45);
    /// assert_eq!((0..0).fold1(|x, y| x * y), None);
    /// ```
    #[deprecated(
        note = "Use [`Iterator::reduce`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.reduce) instead",
        since = "0.10.2"
    )]
    fn fold1<F>(mut self, f: F) -> Option<Self::Item>
    where
        F: FnMut(Self::Item, Self::Item) -> Self::Item,
        Self: Sized,
    {
        self.next().map(move |x| self.fold(x, f))
    }

    /// Accumulate the elements in the iterator in a tree-like manner.
    ///
    /// You can think of it as, while there's more than one item, repeatedly
    /// combining adjacent items.  It does so in bottom-up-merge-sort order,
    /// however, so that it needs only logarithmic stack space.
    ///
    /// This produces a call tree like the following (where the calls under
    /// an item are done after reading that item):
    ///
    /// ```text
    /// 1 2 3 4 5 6 7
    /// │ │ │ │ │ │ │
    /// └─f └─f └─f │
    ///   │   │   │ │
    ///   └───f   └─f
    ///       │     │
    ///       └─────f
    /// ```
    ///
    /// Which, for non-associative functions, will typically produce a different
    /// result than the linear call tree used by [`Iterator::reduce`]:
    ///
    /// ```text
    /// 1 2 3 4 5 6 7
    /// │ │ │ │ │ │ │
    /// └─f─f─f─f─f─f
    /// ```
    ///
    /// If `f` is associative you should also decide carefully:
    ///
    /// For an iterator producing `n` elements, both [`Iterator::reduce`] and `tree_reduce` will
    /// call `f` `n - 1` times. However, `tree_reduce` will call `f` on earlier intermediate
    /// results, which is beneficial for `f` that allocate and produce longer results for longer
    /// arguments. For example if `f` combines arguments using `format!`, then `tree_reduce` will
    /// operate on average on shorter arguments resulting in less bytes being allocated overall.
    ///
    /// Moreover, the output of `tree_reduce` is preferable to that of [`Iterator::reduce`] in
    /// certain cases. For example, building a binary search tree using `tree_reduce` will result in
    /// a balanced tree with height `O(ln(n))`, while [`Iterator::reduce`] will output a tree with
    /// height `O(n)`, essentially a linked list.
    ///
    /// If `f` does not benefit from such a reordering, like `u32::wrapping_add`, prefer the
    /// normal [`Iterator::reduce`] instead since it will most likely result in the generation of
    /// simpler code because the compiler is able to optimize it.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let f = |a: String, b: String| {
    ///     format!("f({a}, {b})")
    /// };
    ///
    /// // The same tree as above
    /// assert_eq!((1..8).map(|x| x.to_string()).tree_reduce(f),
    ///            Some(String::from("f(f(f(1, 2), f(3, 4)), f(f(5, 6), 7))")));
    ///
    /// // Like reduce, an empty iterator produces None
    /// assert_eq!((0..0).tree_reduce(|x, y| x * y), None);
    ///
    /// // tree_reduce matches reduce for associative operations...
    /// assert_eq!((0..10).tree_reduce(|x, y| x + y),
    ///     (0..10).reduce(|x, y| x + y));
    ///
    /// // ...but not for non-associative ones
    /// assert_ne!((0..10).tree_reduce(|x, y| x - y),
    ///     (0..10).reduce(|x, y| x - y));
    ///
    /// let mut total_len_reduce = 0;
    /// let reduce_res = (1..100).map(|x| x.to_string())
    ///     .reduce(|a, b| {
    ///         let r = f(a, b);
    ///         total_len_reduce += r.len();
    ///         r
    ///     })
    ///     .unwrap();
    ///
    /// let mut total_len_tree_reduce = 0;
    /// let tree_reduce_res = (1..100).map(|x| x.to_string())
    ///     .tree_reduce(|a, b| {
    ///         let r = f(a, b);
    ///         total_len_tree_reduce += r.len();
    ///         r
    ///     })
    ///     .unwrap();
    ///
    /// assert_eq!(total_len_reduce, 33299);
    /// assert_eq!(total_len_tree_reduce, 4228);
    /// assert_eq!(reduce_res.len(), tree_reduce_res.len());
    /// ```
    fn tree_reduce<F>(mut self, mut f: F) -> Option<Self::Item>
    where
        F: FnMut(Self::Item, Self::Item) -> Self::Item,
        Self: Sized,
    {
        type State<T> = Result<T, Option<T>>;

        fn inner0<T, II, FF>(it: &mut II, f: &mut FF) -> State<T>
        where
            II: Iterator<Item = T>,
            FF: FnMut(T, T) -> T,
        {
            // This function could be replaced with `it.next().ok_or(None)`,
            // but half the useful tree_reduce work is combining adjacent items,
            // so put that in a form that LLVM is more likely to optimize well.

            let a = if let Some(v) = it.next() {
                v
            } else {
                return Err(None);
            };
            let b = if let Some(v) = it.next() {
                v
            } else {
                return Err(Some(a));
            };
            Ok(f(a, b))
        }

        fn inner<T, II, FF>(stop: usize, it: &mut II, f: &mut FF) -> State<T>
        where
            II: Iterator<Item = T>,
            FF: FnMut(T, T) -> T,
        {
            let mut x = inner0(it, f)?;
            for height in 0..stop {
                // Try to get another tree the same size with which to combine it,
                // creating a new tree that's twice as big for next time around.
                let next = if height == 0 {
                    inner0(it, f)
                } else {
                    inner(height, it, f)
                };
                match next {
                    Ok(y) => x = f(x, y),

                    // If we ran out of items, combine whatever we did manage
                    // to get.  It's better combined with the current value
                    // than something in a parent frame, because the tree in
                    // the parent is always as least as big as this one.
                    Err(None) => return Err(Some(x)),
                    Err(Some(y)) => return Err(Some(f(x, y))),
                }
            }
            Ok(x)
        }

        match inner(usize::MAX, &mut self, &mut f) {
            Err(x) => x,
            _ => unreachable!(),
        }
    }

    /// See [`.tree_reduce()`](Itertools::tree_reduce).
    #[deprecated(note = "Use .tree_reduce() instead", since = "0.13.0")]
    fn tree_fold1<F>(self, f: F) -> Option<Self::Item>
    where
        F: FnMut(Self::Item, Self::Item) -> Self::Item,
        Self: Sized,
    {
        self.tree_reduce(f)
    }

    /// An iterator method that applies a function, producing a single, final value.
    ///
    /// `fold_while()` is basically equivalent to [`Iterator::fold`] but with additional support for
    /// early exit via short-circuiting.
    ///
    /// ```
    /// use itertools::Itertools;
    /// use itertools::FoldWhile::{Continue, Done};
    ///
    /// let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    ///
    /// let mut result = 0;
    ///
    /// // for loop:
    /// for i in &numbers {
    ///     if *i > 5 {
    ///         break;
    ///     }
    ///     result = result + i;
    /// }
    ///
    /// // fold:
    /// let result2 = numbers.iter().fold(0, |acc, x| {
    ///     if *x > 5 { acc } else { acc + x }
    /// });
    ///
    /// // fold_while:
    /// let result3 = numbers.iter().fold_while(0, |acc, x| {
    ///     if *x > 5 { Done(acc) } else { Continue(acc + x) }
    /// }).into_inner();
    ///
    /// // they're the same
    /// assert_eq!(result, result2);
    /// assert_eq!(result2, result3);
    /// ```
    ///
    /// The big difference between the computations of `result2` and `result3` is that while
    /// `fold()` called the provided closure for every item of the callee iterator,
    /// `fold_while()` actually stopped iterating as soon as it encountered `Fold::Done(_)`.
    fn fold_while<B, F>(&mut self, init: B, mut f: F) -> FoldWhile<B>
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> FoldWhile<B>,
    {
        use Result::{Err as Break, Ok as Continue};

        let result = self.try_fold(
            init,
            #[inline(always)]
            |acc, v| match f(acc, v) {
                FoldWhile::Continue(acc) => Continue(acc),
                FoldWhile::Done(acc) => Break(acc),
            },
        );

        match result {
            Continue(acc) => FoldWhile::Continue(acc),
            Break(acc) => FoldWhile::Done(acc),
        }
    }

    /// Iterate over the entire iterator and add all the elements.
    ///
    /// An empty iterator returns `None`, otherwise `Some(sum)`.
    ///
    /// # Panics
    ///
    /// When calling `sum1()` and a primitive integer type is being returned, this
    /// method will panic if the computation overflows and debug assertions are
    /// enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let empty_sum = (1..1).sum1::<i32>();
    /// assert_eq!(empty_sum, None);
    ///
    /// let nonempty_sum = (1..11).sum1::<i32>();
    /// assert_eq!(nonempty_sum, Some(55));
    /// ```
    fn sum1<S>(mut self) -> Option<S>
    where
        Self: Sized,
        S: std::iter::Sum<Self::Item>,
    {
        self.next().map(|first| once(first).chain(self).sum())
    }

    /// Iterate over the entire iterator and multiply all the elements.
    ///
    /// An empty iterator returns `None`, otherwise `Some(product)`.
    ///
    /// # Panics
    ///
    /// When calling `product1()` and a primitive integer type is being returned,
    /// method will panic if the computation overflows and debug assertions are
    /// enabled.
    ///
    /// # Examples
    /// ```
    /// use itertools::Itertools;
    ///
    /// let empty_product = (1..1).product1::<i32>();
    /// assert_eq!(empty_product, None);
    ///
    /// let nonempty_product = (1..11).product1::<i32>();
    /// assert_eq!(nonempty_product, Some(3628800));
    /// ```
    fn product1<P>(mut self) -> Option<P>
    where
        Self: Sized,
        P: std::iter::Product<Self::Item>,
    {
        self.next().map(|first| once(first).chain(self).product())
    }

    /// Sort all iterator elements into a new iterator in ascending order.
    ///
    /// **Note:** This consumes the entire iterator, uses the
    /// [`slice::sort_unstable`] method and returns the result as a new
    /// iterator that owns its elements.
    ///
    /// This sort is unstable (i.e., may reorder equal elements).
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // sort the letters of the text in ascending order
    /// let text = "bdacfe";
    /// itertools::assert_equal(text.chars().sorted_unstable(),
    ///                         "abcdef".chars());
    /// ```
    #[cfg(feature = "use_alloc")]
    fn sorted_unstable(self) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        // Use .sort_unstable() directly since it is not quite identical with
        // .sort_by(Ord::cmp)
        let mut v = Vec::from_iter(self);
        v.sort_unstable();
        v.into_iter()
    }

    /// Sort all iterator elements into a new iterator in ascending order.
    ///
    /// **Note:** This consumes the entire iterator, uses the
    /// [`slice::sort_unstable_by`] method and returns the result as a new
    /// iterator that owns its elements.
    ///
    /// This sort is unstable (i.e., may reorder equal elements).
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // sort people in descending order by age
    /// let people = vec![("Jane", 20), ("John", 18), ("Jill", 30), ("Jack", 27)];
    ///
    /// let oldest_people_first = people
    ///     .into_iter()
    ///     .sorted_unstable_by(|a, b| Ord::cmp(&b.1, &a.1))
    ///     .map(|(person, _age)| person);
    ///
    /// itertools::assert_equal(oldest_people_first,
    ///                         vec!["Jill", "Jack", "Jane", "John"]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn sorted_unstable_by<F>(self, cmp: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        let mut v = Vec::from_iter(self);
        v.sort_unstable_by(cmp);
        v.into_iter()
    }

    /// Sort all iterator elements into a new iterator in ascending order.
    ///
    /// **Note:** This consumes the entire iterator, uses the
    /// [`slice::sort_unstable_by_key`] method and returns the result as a new
    /// iterator that owns its elements.
    ///
    /// This sort is unstable (i.e., may reorder equal elements).
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // sort people in descending order by age
    /// let people = vec![("Jane", 20), ("John", 18), ("Jill", 30), ("Jack", 27)];
    ///
    /// let oldest_people_first = people
    ///     .into_iter()
    ///     .sorted_unstable_by_key(|x| -x.1)
    ///     .map(|(person, _age)| person);
    ///
    /// itertools::assert_equal(oldest_people_first,
    ///                         vec!["Jill", "Jack", "Jane", "John"]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn sorted_unstable_by_key<K, F>(self, f: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        K: Ord,
        F: FnMut(&Self::Item) -> K,
    {
        let mut v = Vec::from_iter(self);
        v.sort_unstable_by_key(f);
        v.into_iter()
    }

    /// Sort all iterator elements into a new iterator in ascending order.
    ///
    /// **Note:** This consumes the entire iterator, uses the
    /// [`slice::sort`] method and returns the result as a new
    /// iterator that owns its elements.
    ///
    /// This sort is stable (i.e., does not reorder equal elements).
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // sort the letters of the text in ascending order
    /// let text = "bdacfe";
    /// itertools::assert_equal(text.chars().sorted(),
    ///                         "abcdef".chars());
    /// ```
    #[cfg(feature = "use_alloc")]
    fn sorted(self) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        // Use .sort() directly since it is not quite identical with
        // .sort_by(Ord::cmp)
        let mut v = Vec::from_iter(self);
        v.sort();
        v.into_iter()
    }

    /// Sort all iterator elements into a new iterator in ascending order.
    ///
    /// **Note:** This consumes the entire iterator, uses the
    /// [`slice::sort_by`] method and returns the result as a new
    /// iterator that owns its elements.
    ///
    /// This sort is stable (i.e., does not reorder equal elements).
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // sort people in descending order by age
    /// let people = vec![("Jane", 20), ("John", 18), ("Jill", 30), ("Jack", 30)];
    ///
    /// let oldest_people_first = people
    ///     .into_iter()
    ///     .sorted_by(|a, b| Ord::cmp(&b.1, &a.1))
    ///     .map(|(person, _age)| person);
    ///
    /// itertools::assert_equal(oldest_people_first,
    ///                         vec!["Jill", "Jack", "Jane", "John"]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn sorted_by<F>(self, cmp: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        let mut v = Vec::from_iter(self);
        v.sort_by(cmp);
        v.into_iter()
    }

    /// Sort all iterator elements into a new iterator in ascending order.
    ///
    /// **Note:** This consumes the entire iterator, uses the
    /// [`slice::sort_by_key`] method and returns the result as a new
    /// iterator that owns its elements.
    ///
    /// This sort is stable (i.e., does not reorder equal elements).
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // sort people in descending order by age
    /// let people = vec![("Jane", 20), ("John", 18), ("Jill", 30), ("Jack", 30)];
    ///
    /// let oldest_people_first = people
    ///     .into_iter()
    ///     .sorted_by_key(|x| -x.1)
    ///     .map(|(person, _age)| person);
    ///
    /// itertools::assert_equal(oldest_people_first,
    ///                         vec!["Jill", "Jack", "Jane", "John"]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn sorted_by_key<K, F>(self, f: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        K: Ord,
        F: FnMut(&Self::Item) -> K,
    {
        let mut v = Vec::from_iter(self);
        v.sort_by_key(f);
        v.into_iter()
    }

    /// Sort all iterator elements into a new iterator in ascending order. The key function is
    /// called exactly once per key.
    ///
    /// **Note:** This consumes the entire iterator, uses the
    /// [`slice::sort_by_cached_key`] method and returns the result as a new
    /// iterator that owns its elements.
    ///
    /// This sort is stable (i.e., does not reorder equal elements).
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // sort people in descending order by age
    /// let people = vec![("Jane", 20), ("John", 18), ("Jill", 30), ("Jack", 30)];
    ///
    /// let oldest_people_first = people
    ///     .into_iter()
    ///     .sorted_by_cached_key(|x| -x.1)
    ///     .map(|(person, _age)| person);
    ///
    /// itertools::assert_equal(oldest_people_first,
    ///                         vec!["Jill", "Jack", "Jane", "John"]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn sorted_by_cached_key<K, F>(self, f: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        K: Ord,
        F: FnMut(&Self::Item) -> K,
    {
        let mut v = Vec::from_iter(self);
        v.sort_by_cached_key(f);
        v.into_iter()
    }

    /// Sort the k smallest elements into a new iterator, in ascending order.
    ///
    /// **Note:** This consumes the entire iterator, and returns the result
    /// as a new iterator that owns its elements.  If the input contains
    /// less than k elements, the result is equivalent to `self.sorted()`.
    ///
    /// This is guaranteed to use `k * sizeof(Self::Item) + O(1)` memory
    /// and `O(n log k)` time, with `n` the number of elements in the input.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// **Note:** This is functionally-equivalent to `self.sorted().take(k)`
    /// but much more efficient.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_smallest = numbers
    ///     .into_iter()
    ///     .k_smallest(5);
    ///
    /// itertools::assert_equal(five_smallest, 0..5);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_smallest(self, k: usize) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        // The stdlib heap has optimised handling of "holes", which is not included in our heap implementation in k_smallest_general.
        // While the difference is unlikely to have practical impact unless `Self::Item` is very large, this method uses the stdlib structure
        // to maintain performance compared to previous versions of the crate.
        use alloc::collections::BinaryHeap;

        if k == 0 {
            self.last();
            return Vec::new().into_iter();
        }
        if k == 1 {
            return self.min().into_iter().collect_vec().into_iter();
        }

        let mut iter = self.fuse();
        let mut heap: BinaryHeap<_> = iter.by_ref().take(k).collect();

        iter.for_each(|i| {
            debug_assert_eq!(heap.len(), k);
            // Equivalent to heap.push(min(i, heap.pop())) but more efficient.
            // This should be done with a single `.peek_mut().unwrap()` but
            //  `PeekMut` sifts-down unconditionally on Rust 1.46.0 and prior.
            if *heap.peek().unwrap() > i {
                *heap.peek_mut().unwrap() = i;
            }
        });

        heap.into_sorted_vec().into_iter()
    }

    /// Sort the k smallest elements into a new iterator using the provided comparison.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// This corresponds to `self.sorted_by(cmp).take(k)` in the same way that
    /// [`k_smallest`](Itertools::k_smallest) corresponds to `self.sorted().take(k)`,
    /// in both semantics and complexity.
    ///
    /// Particularly, a custom heap implementation ensures the comparison is not cloned.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_smallest = numbers
    ///     .into_iter()
    ///     .k_smallest_by(5, |a, b| (a % 7).cmp(&(b % 7)).then(a.cmp(b)));
    ///
    /// itertools::assert_equal(five_smallest, vec![0, 7, 14, 1, 8]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_smallest_by<F>(self, k: usize, cmp: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        k_smallest::k_smallest_general(self, k, cmp).into_iter()
    }

    /// Return the elements producing the k smallest outputs of the provided function.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// This corresponds to `self.sorted_by_key(key).take(k)` in the same way that
    /// [`k_smallest`](Itertools::k_smallest) corresponds to `self.sorted().take(k)`,
    /// in both semantics and complexity.
    ///
    /// Particularly, a custom heap implementation ensures the comparison is not cloned.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_smallest = numbers
    ///     .into_iter()
    ///     .k_smallest_by_key(5, |n| (n % 7, *n));
    ///
    /// itertools::assert_equal(five_smallest, vec![0, 7, 14, 1, 8]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_smallest_by_key<F, K>(self, k: usize, key: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> K,
        K: Ord,
    {
        self.k_smallest_by(k, k_smallest::key_to_cmp(key))
    }

    /// Sort the k smallest elements into a new iterator, in ascending order, relaxing the amount of memory required.
    ///
    /// **Note:** This consumes the entire iterator, and returns the result
    /// as a new iterator that owns its elements.  If the input contains
    /// less than k elements, the result is equivalent to `self.sorted()`.
    ///
    /// This is guaranteed to use `2 * k * sizeof(Self::Item) + O(1)` memory
    /// and `O(n + k log k)` time, with `n` the number of elements in the input,
    /// meaning it uses more memory than the minimum obtained by [`k_smallest`](Itertools::k_smallest)
    /// but achieves linear time in the number of elements.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// **Note:** This is functionally-equivalent to `self.sorted().take(k)`
    /// but much more efficient.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_smallest = numbers
    ///     .into_iter()
    ///     .k_smallest_relaxed(5);
    ///
    /// itertools::assert_equal(five_smallest, 0..5);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_smallest_relaxed(self, k: usize) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        self.k_smallest_relaxed_by(k, Ord::cmp)
    }

    /// Sort the k smallest elements into a new iterator using the provided comparison, relaxing the amount of memory required.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// This corresponds to `self.sorted_by(cmp).take(k)` in the same way that
    /// [`k_smallest_relaxed`](Itertools::k_smallest_relaxed) corresponds to `self.sorted().take(k)`,
    /// in both semantics and complexity.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_smallest = numbers
    ///     .into_iter()
    ///     .k_smallest_relaxed_by(5, |a, b| (a % 7).cmp(&(b % 7)).then(a.cmp(b)));
    ///
    /// itertools::assert_equal(five_smallest, vec![0, 7, 14, 1, 8]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_smallest_relaxed_by<F>(self, k: usize, cmp: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        k_smallest::k_smallest_relaxed_general(self, k, cmp).into_iter()
    }

    /// Return the elements producing the k smallest outputs of the provided function, relaxing the amount of memory required.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// This corresponds to `self.sorted_by_key(key).take(k)` in the same way that
    /// [`k_smallest_relaxed`](Itertools::k_smallest_relaxed) corresponds to `self.sorted().take(k)`,
    /// in both semantics and complexity.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_smallest = numbers
    ///     .into_iter()
    ///     .k_smallest_relaxed_by_key(5, |n| (n % 7, *n));
    ///
    /// itertools::assert_equal(five_smallest, vec![0, 7, 14, 1, 8]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_smallest_relaxed_by_key<F, K>(self, k: usize, key: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> K,
        K: Ord,
    {
        self.k_smallest_relaxed_by(k, k_smallest::key_to_cmp(key))
    }

    /// Sort the k largest elements into a new iterator, in descending order.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// It is semantically equivalent to [`k_smallest`](Itertools::k_smallest)
    /// with a reversed `Ord`.
    /// However, this is implemented with a custom binary heap which does not
    /// have the same performance characteristics for very large `Self::Item`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_largest = numbers
    ///     .into_iter()
    ///     .k_largest(5);
    ///
    /// itertools::assert_equal(five_largest, vec![14, 13, 12, 11, 10]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_largest(self, k: usize) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        self.k_largest_by(k, Self::Item::cmp)
    }

    /// Sort the k largest elements into a new iterator using the provided comparison.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// Functionally equivalent to [`k_smallest_by`](Itertools::k_smallest_by)
    /// with a reversed `Ord`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_largest = numbers
    ///     .into_iter()
    ///     .k_largest_by(5, |a, b| (a % 7).cmp(&(b % 7)).then(a.cmp(b)));
    ///
    /// itertools::assert_equal(five_largest, vec![13, 6, 12, 5, 11]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_largest_by<F>(self, k: usize, mut cmp: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        self.k_smallest_by(k, move |a, b| cmp(b, a))
    }

    /// Return the elements producing the k largest outputs of the provided function.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// Functionally equivalent to [`k_smallest_by_key`](Itertools::k_smallest_by_key)
    /// with a reversed `Ord`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_largest = numbers
    ///     .into_iter()
    ///     .k_largest_by_key(5, |n| (n % 7, *n));
    ///
    /// itertools::assert_equal(five_largest, vec![13, 6, 12, 5, 11]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_largest_by_key<F, K>(self, k: usize, key: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> K,
        K: Ord,
    {
        self.k_largest_by(k, k_smallest::key_to_cmp(key))
    }

    /// Sort the k largest elements into a new iterator, in descending order, relaxing the amount of memory required.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// It is semantically equivalent to [`k_smallest_relaxed`](Itertools::k_smallest_relaxed)
    /// with a reversed `Ord`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_largest = numbers
    ///     .into_iter()
    ///     .k_largest_relaxed(5);
    ///
    /// itertools::assert_equal(five_largest, vec![14, 13, 12, 11, 10]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_largest_relaxed(self, k: usize) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        self.k_largest_relaxed_by(k, Self::Item::cmp)
    }

    /// Sort the k largest elements into a new iterator using the provided comparison, relaxing the amount of memory required.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// Functionally equivalent to [`k_smallest_relaxed_by`](Itertools::k_smallest_relaxed_by)
    /// with a reversed `Ord`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_largest = numbers
    ///     .into_iter()
    ///     .k_largest_relaxed_by(5, |a, b| (a % 7).cmp(&(b % 7)).then(a.cmp(b)));
    ///
    /// itertools::assert_equal(five_largest, vec![13, 6, 12, 5, 11]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_largest_relaxed_by<F>(self, k: usize, mut cmp: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        self.k_smallest_relaxed_by(k, move |a, b| cmp(b, a))
    }

    /// Return the elements producing the k largest outputs of the provided function, relaxing the amount of memory required.
    ///
    /// The sorted iterator, if directly collected to a `Vec`, is converted
    /// without any extra copying or allocation cost.
    ///
    /// Functionally equivalent to [`k_smallest_relaxed_by_key`](Itertools::k_smallest_relaxed_by_key)
    /// with a reversed `Ord`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// // A random permutation of 0..15
    /// let numbers = vec![6, 9, 1, 14, 0, 4, 8, 7, 11, 2, 10, 3, 13, 12, 5];
    ///
    /// let five_largest = numbers
    ///     .into_iter()
    ///     .k_largest_relaxed_by_key(5, |n| (n % 7, *n));
    ///
    /// itertools::assert_equal(five_largest, vec![13, 6, 12, 5, 11]);
    /// ```
    #[cfg(feature = "use_alloc")]
    fn k_largest_relaxed_by_key<F, K>(self, k: usize, key: F) -> VecIntoIter<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> K,
        K: Ord,
    {
        self.k_largest_relaxed_by(k, k_smallest::key_to_cmp(key))
    }

    /// Consumes the iterator and return an iterator of the last `n` elements.
    ///
    /// The iterator, if directly collected to a `VecDeque`, is converted
    /// without any extra copying or allocation cost.
    /// If directly collected to a `Vec`, it may need some data movement
    /// but no re-allocation.
    ///
    /// ```
    /// use itertools::{assert_equal, Itertools};
    ///
    /// let v = vec![5, 9, 8, 4, 2, 12, 0];
    /// assert_equal(v.iter().tail(3), &[2, 12, 0]);
    /// assert_equal(v.iter().tail(10), &v);
    ///
    /// assert_equal(v.iter().tail(1), v.iter().last());
    ///
    /// assert_equal((0..100).tail(10), 90..100);
    ///
    /// assert_equal((0..100).filter(|x| x % 3 == 0).tail(10), (72..100).step_by(3));
    /// ```
    ///
    /// For double ended iterators without side-effects, you might prefer
    /// `.rev().take(n).rev()` to have a similar result (lazy and non-allocating)
    /// without consuming the entire iterator.
    #[cfg(feature = "use_alloc")]
    fn tail(self, n: usize) -> VecDequeIntoIter<Self::Item>
    where
        Self: Sized,
    {
        match n {
            0 => {
                self.last();
                VecDeque::new()
            }
            1 => self.last().into_iter().collect(),
            _ => {
                // Skip the starting part of the iterator if possible.
                let (low, _) = self.size_hint();
                let mut iter = self.fuse().skip(low.saturating_sub(n));
                // TODO: If VecDeque has a more efficient method than
                // `.pop_front();.push_back(val)` in the future then maybe revisit this.
                let mut data: Vec<_> = iter.by_ref().take(n).collect();
                // Update `data` cyclically.
                let idx = iter.fold(0, |i, val| {
                    debug_assert_eq!(data.len(), n);
                    data[i] = val;
                    if i + 1 == n {
                        0
                    } else {
                        i + 1
                    }
                });
                // Respect the insertion order, efficiently.
                let mut data = VecDeque::from(data);
                data.rotate_left(idx);
                data
            }
        }
        .into_iter()
    }

    /// Collect all iterator elements into one of two
    /// partitions. Unlike [`Iterator::partition`], each partition may
    /// have a distinct type.
    ///
    /// ```
    /// use itertools::{Itertools, Either};
    ///
    /// let successes_and_failures = vec![Ok(1), Err(false), Err(true), Ok(2)];
    ///
    /// let (successes, failures): (Vec<_>, Vec<_>) = successes_and_failures
    ///     .into_iter()
    ///     .partition_map(|r| {
    ///         match r {
    ///             Ok(v) => Either::Left(v),
    ///             Err(v) => Either::Right(v),
    ///         }
    ///     });
    ///
    /// assert_eq!(successes, [1, 2]);
    /// assert_eq!(failures, [false, true]);
    /// ```
    fn partition_map<A, B, F, L, R>(self, mut predicate: F) -> (A, B)
    where
        Self: Sized,
        F: FnMut(Self::Item) -> Either<L, R>,
        A: Default + Extend<L>,
        B: Default + Extend<R>,
    {
        let mut left = A::default();
        let mut right = B::default();

        self.for_each(|val| match predicate(val) {
            Either::Left(v) => left.extend(Some(v)),
            Either::Right(v) => right.extend(Some(v)),
        });

        (left, right)
    }

    /// Partition a sequence of `Result`s into one list of all the `Ok` elements
    /// and another list of all the `Err` elements.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let successes_and_failures = vec![Ok(1), Err(false), Err(true), Ok(2)];
    ///
    /// let (successes, failures): (Vec<_>, Vec<_>) = successes_and_failures
    ///     .into_iter()
    ///     .partition_result();
    ///
    /// assert_eq!(successes, [1, 2]);
    /// assert_eq!(failures, [false, true]);
    /// ```
    fn partition_result<A, B, T, E>(self) -> (A, B)
    where
        Self: Iterator<Item = Result<T, E>> + Sized,
        A: Default + Extend<T>,
        B: Default + Extend<E>,
    {
        self.partition_map(|r| match r {
            Ok(v) => Either::Left(v),
            Err(v) => Either::Right(v),
        })
    }

    /// Return a `HashMap` of keys mapped to `Vec`s of values. Keys and values
    /// are taken from `(Key, Value)` tuple pairs yielded by the input iterator.
    ///
    /// Essentially a shorthand for `.into_grouping_map().collect::<Vec<_>>()`.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let data = vec![(0, 10), (2, 12), (3, 13), (0, 20), (3, 33), (2, 42)];
    /// let lookup = data.into_iter().into_group_map();
    ///
    /// assert_eq!(lookup[&0], vec![10, 20]);
    /// assert_eq!(lookup.get(&1), None);
    /// assert_eq!(lookup[&2], vec![12, 42]);
    /// assert_eq!(lookup[&3], vec![13, 33]);
    /// ```
    #[cfg(feature = "use_std")]
    fn into_group_map<K, V>(self) -> HashMap<K, Vec<V>>
    where
        Self: Iterator<Item = (K, V)> + Sized,
        K: Hash + Eq,
    {
        group_map::into_group_map(self)
    }

    /// Return a `HashMap` of keys mapped to `Vec`s of values. The key is specified
    /// in the closure. The values are taken from the input iterator.
    ///
    /// Essentially a shorthand for `.into_grouping_map_by(f).collect::<Vec<_>>()`.
    ///
    /// ```
    /// use itertools::Itertools;
    /// use std::collections::HashMap;
    ///
    /// let data = vec![(0, 10), (2, 12), (3, 13), (0, 20), (3, 33), (2, 42)];
    /// let lookup: HashMap<u32,Vec<(u32, u32)>> =
    ///     data.clone().into_iter().into_group_map_by(|a| a.0);
    ///
    /// assert_eq!(lookup[&0], vec![(0,10), (0,20)]);
    /// assert_eq!(lookup.get(&1), None);
    /// assert_eq!(lookup[&2], vec![(2,12), (2,42)]);
    /// assert_eq!(lookup[&3], vec![(3,13), (3,33)]);
    ///
    /// assert_eq!(
    ///     data.into_iter()
    ///         .into_group_map_by(|x| x.0)
    ///         .into_iter()
    ///         .map(|(key, values)| (key, values.into_iter().fold(0,|acc, (_,v)| acc + v )))
    ///         .collect::<HashMap<u32,u32>>()[&0],
    ///     30,
    /// );
    /// ```
    #[cfg(feature = "use_std")]
    fn into_group_map_by<K, V, F>(self, f: F) -> HashMap<K, Vec<V>>
    where
        Self: Iterator<Item = V> + Sized,
        K: Hash + Eq,
        F: FnMut(&V) -> K,
    {
        group_map::into_group_map_by(self, f)
    }

    /// Constructs a `GroupingMap` to be used later with one of the efficient
    /// group-and-fold operations it allows to perform.
    ///
    /// The input iterator must yield item in the form of `(K, V)` where the
    /// value of type `K` will be used as key to identify the groups and the
    /// value of type `V` as value for the folding operation.
    ///
    /// See [`GroupingMap`] for more informations
    /// on what operations are available.
    #[cfg(feature = "use_std")]
    fn into_grouping_map<K, V>(self) -> GroupingMap<Self>
    where
        Self: Iterator<Item = (K, V)> + Sized,
        K: Hash + Eq,
    {
        grouping_map::new(self)
    }

    /// Constructs a `GroupingMap` to be used later with one of the efficient
    /// group-and-fold operations it allows to perform.
    ///
    /// The values from this iterator will be used as values for the folding operation
    /// while the keys will be obtained from the values by calling `key_mapper`.
    ///
    /// See [`GroupingMap`] for more informations
    /// on what operations are available.
    #[cfg(feature = "use_std")]
    fn into_grouping_map_by<K, V, F>(self, key_mapper: F) -> GroupingMapBy<Self, F>
    where
        Self: Iterator<Item = V> + Sized,
        K: Hash + Eq,
        F: FnMut(&V) -> K,
    {
        grouping_map::new(grouping_map::new_map_for_grouping(self, key_mapper))
    }

    /// Return all minimum elements of an iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().min_set(), Vec::<&i32>::new());
    ///
    /// let a = [1];
    /// assert_eq!(a.iter().min_set(), vec![&1]);
    ///
    /// let a = [1, 2, 3, 4, 5];
    /// assert_eq!(a.iter().min_set(), vec![&1]);
    ///
    /// let a = [1, 1, 1, 1];
    /// assert_eq!(a.iter().min_set(), vec![&1, &1, &1, &1]);
    /// ```
    ///
    /// The elements can be floats but no particular result is guaranteed
    /// if an element is NaN.
    #[cfg(feature = "use_alloc")]
    fn min_set(self) -> Vec<Self::Item>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        extrema_set::min_set_impl(self, |_| (), |x, y, _, _| x.cmp(y))
    }

    /// Return all minimum elements of an iterator, as determined by
    /// the specified function.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::cmp::Ordering;
    /// use itertools::Itertools;
    ///
    /// let a: [(i32, i32); 0] = [];
    /// assert_eq!(a.iter().min_set_by(|_, _| Ordering::Equal), Vec::<&(i32, i32)>::new());
    ///
    /// let a = [(1, 2)];
    /// assert_eq!(a.iter().min_set_by(|&&(k1,_), &&(k2, _)| k1.cmp(&k2)), vec![&(1, 2)]);
    ///
    /// let a = [(1, 2), (2, 2), (3, 9), (4, 8), (5, 9)];
    /// assert_eq!(a.iter().min_set_by(|&&(_,k1), &&(_,k2)| k1.cmp(&k2)), vec![&(1, 2), &(2, 2)]);
    ///
    /// let a = [(1, 2), (1, 3), (1, 4), (1, 5)];
    /// assert_eq!(a.iter().min_set_by(|&&(k1,_), &&(k2, _)| k1.cmp(&k2)), vec![&(1, 2), &(1, 3), &(1, 4), &(1, 5)]);
    /// ```
    ///
    /// The elements can be floats but no particular result is guaranteed
    /// if an element is NaN.
    #[cfg(feature = "use_alloc")]
    fn min_set_by<F>(self, mut compare: F) -> Vec<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        extrema_set::min_set_impl(self, |_| (), |x, y, _, _| compare(x, y))
    }

    /// Return all minimum elements of an iterator, as determined by
    /// the specified function.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a: [(i32, i32); 0] = [];
    /// assert_eq!(a.iter().min_set_by_key(|_| ()), Vec::<&(i32, i32)>::new());
    ///
    /// let a = [(1, 2)];
    /// assert_eq!(a.iter().min_set_by_key(|&&(k,_)| k), vec![&(1, 2)]);
    ///
    /// let a = [(1, 2), (2, 2), (3, 9), (4, 8), (5, 9)];
    /// assert_eq!(a.iter().min_set_by_key(|&&(_, k)| k), vec![&(1, 2), &(2, 2)]);
    ///
    /// let a = [(1, 2), (1, 3), (1, 4), (1, 5)];
    /// assert_eq!(a.iter().min_set_by_key(|&&(k, _)| k), vec![&(1, 2), &(1, 3), &(1, 4), &(1, 5)]);
    /// ```
    ///
    /// The elements can be floats but no particular result is guaranteed
    /// if an element is NaN.
    #[cfg(feature = "use_alloc")]
    fn min_set_by_key<K, F>(self, key: F) -> Vec<Self::Item>
    where
        Self: Sized,
        K: Ord,
        F: FnMut(&Self::Item) -> K,
    {
        extrema_set::min_set_impl(self, key, |_, _, kx, ky| kx.cmp(ky))
    }

    /// Return all maximum elements of an iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().max_set(), Vec::<&i32>::new());
    ///
    /// let a = [1];
    /// assert_eq!(a.iter().max_set(), vec![&1]);
    ///
    /// let a = [1, 2, 3, 4, 5];
    /// assert_eq!(a.iter().max_set(), vec![&5]);
    ///
    /// let a = [1, 1, 1, 1];
    /// assert_eq!(a.iter().max_set(), vec![&1, &1, &1, &1]);
    /// ```
    ///
    /// The elements can be floats but no particular result is guaranteed
    /// if an element is NaN.
    #[cfg(feature = "use_alloc")]
    fn max_set(self) -> Vec<Self::Item>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        extrema_set::max_set_impl(self, |_| (), |x, y, _, _| x.cmp(y))
    }

    /// Return all maximum elements of an iterator, as determined by
    /// the specified function.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::cmp::Ordering;
    /// use itertools::Itertools;
    ///
    /// let a: [(i32, i32); 0] = [];
    /// assert_eq!(a.iter().max_set_by(|_, _| Ordering::Equal), Vec::<&(i32, i32)>::new());
    ///
    /// let a = [(1, 2)];
    /// assert_eq!(a.iter().max_set_by(|&&(k1,_), &&(k2, _)| k1.cmp(&k2)), vec![&(1, 2)]);
    ///
    /// let a = [(1, 2), (2, 2), (3, 9), (4, 8), (5, 9)];
    /// assert_eq!(a.iter().max_set_by(|&&(_,k1), &&(_,k2)| k1.cmp(&k2)), vec![&(3, 9), &(5, 9)]);
    ///
    /// let a = [(1, 2), (1, 3), (1, 4), (1, 5)];
    /// assert_eq!(a.iter().max_set_by(|&&(k1,_), &&(k2, _)| k1.cmp(&k2)), vec![&(1, 2), &(1, 3), &(1, 4), &(1, 5)]);
    /// ```
    ///
    /// The elements can be floats but no particular result is guaranteed
    /// if an element is NaN.
    #[cfg(feature = "use_alloc")]
    fn max_set_by<F>(self, mut compare: F) -> Vec<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        extrema_set::max_set_impl(self, |_| (), |x, y, _, _| compare(x, y))
    }

    /// Return all maximum elements of an iterator, as determined by
    /// the specified function.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a: [(i32, i32); 0] = [];
    /// assert_eq!(a.iter().max_set_by_key(|_| ()), Vec::<&(i32, i32)>::new());
    ///
    /// let a = [(1, 2)];
    /// assert_eq!(a.iter().max_set_by_key(|&&(k,_)| k), vec![&(1, 2)]);
    ///
    /// let a = [(1, 2), (2, 2), (3, 9), (4, 8), (5, 9)];
    /// assert_eq!(a.iter().max_set_by_key(|&&(_, k)| k), vec![&(3, 9), &(5, 9)]);
    ///
    /// let a = [(1, 2), (1, 3), (1, 4), (1, 5)];
    /// assert_eq!(a.iter().max_set_by_key(|&&(k, _)| k), vec![&(1, 2), &(1, 3), &(1, 4), &(1, 5)]);
    /// ```
    ///
    /// The elements can be floats but no particular result is guaranteed
    /// if an element is NaN.
    #[cfg(feature = "use_alloc")]
    fn max_set_by_key<K, F>(self, key: F) -> Vec<Self::Item>
    where
        Self: Sized,
        K: Ord,
        F: FnMut(&Self::Item) -> K,
    {
        extrema_set::max_set_impl(self, key, |_, _, kx, ky| kx.cmp(ky))
    }

    /// Return the minimum and maximum elements in the iterator.
    ///
    /// The return type `MinMaxResult` is an enum of three variants:
    ///
    /// - `NoElements` if the iterator is empty.
    /// - `OneElement(x)` if the iterator has exactly one element.
    /// - `MinMax(x, y)` is returned otherwise, where `x <= y`. Two
    ///    values are equal if and only if there is more than one
    ///    element in the iterator and all elements are equal.
    ///
    /// On an iterator of length `n`, `minmax` does `1.5 * n` comparisons,
    /// and so is faster than calling `min` and `max` separately which does
    /// `2 * n` comparisons.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    /// use itertools::MinMaxResult::{NoElements, OneElement, MinMax};
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().minmax(), NoElements);
    ///
    /// let a = [1];
    /// assert_eq!(a.iter().minmax(), OneElement(&1));
    ///
    /// let a = [1, 2, 3, 4, 5];
    /// assert_eq!(a.iter().minmax(), MinMax(&1, &5));
    ///
    /// let a = [1, 1, 1, 1];
    /// assert_eq!(a.iter().minmax(), MinMax(&1, &1));
    /// ```
    ///
    /// The elements can be floats but no particular result is guaranteed
    /// if an element is NaN.
    fn minmax(self) -> MinMaxResult<Self::Item>
    where
        Self: Sized,
        Self::Item: PartialOrd,
    {
        minmax::minmax_impl(self, |_| (), |x, y, _, _| x < y)
    }

    /// Return the minimum and maximum element of an iterator, as determined by
    /// the specified function.
    ///
    /// The return value is a variant of [`MinMaxResult`] like for [`.minmax()`](Itertools::minmax).
    ///
    /// For the minimum, the first minimal element is returned.  For the maximum,
    /// the last maximal element wins.  This matches the behavior of the standard
    /// [`Iterator::min`] and [`Iterator::max`] methods.
    ///
    /// The keys can be floats but no particular result is guaranteed
    /// if a key is NaN.
    fn minmax_by_key<K, F>(self, key: F) -> MinMaxResult<Self::Item>
    where
        Self: Sized,
        K: PartialOrd,
        F: FnMut(&Self::Item) -> K,
    {
        minmax::minmax_impl(self, key, |_, _, xk, yk| xk < yk)
    }

    /// Return the minimum and maximum element of an iterator, as determined by
    /// the specified comparison function.
    ///
    /// The return value is a variant of [`MinMaxResult`] like for [`.minmax()`](Itertools::minmax).
    ///
    /// For the minimum, the first minimal element is returned.  For the maximum,
    /// the last maximal element wins.  This matches the behavior of the standard
    /// [`Iterator::min`] and [`Iterator::max`] methods.
    fn minmax_by<F>(self, mut compare: F) -> MinMaxResult<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        minmax::minmax_impl(self, |_| (), |x, y, _, _| Ordering::Less == compare(x, y))
    }

    /// Return the position of the maximum element in the iterator.
    ///
    /// If several elements are equally maximum, the position of the
    /// last of them is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().position_max(), None);
    ///
    /// let a = [-3, 0, 1, 5, -10];
    /// assert_eq!(a.iter().position_max(), Some(3));
    ///
    /// let a = [1, 1, -1, -1];
    /// assert_eq!(a.iter().position_max(), Some(1));
    /// ```
    fn position_max(self) -> Option<usize>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        self.enumerate()
            .max_by(|x, y| Ord::cmp(&x.1, &y.1))
            .map(|x| x.0)
    }

    /// Return the position of the maximum element in the iterator, as
    /// determined by the specified function.
    ///
    /// If several elements are equally maximum, the position of the
    /// last of them is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().position_max_by_key(|x| x.abs()), None);
    ///
    /// let a = [-3_i32, 0, 1, 5, -10];
    /// assert_eq!(a.iter().position_max_by_key(|x| x.abs()), Some(4));
    ///
    /// let a = [1_i32, 1, -1, -1];
    /// assert_eq!(a.iter().position_max_by_key(|x| x.abs()), Some(3));
    /// ```
    fn position_max_by_key<K, F>(self, mut key: F) -> Option<usize>
    where
        Self: Sized,
        K: Ord,
        F: FnMut(&Self::Item) -> K,
    {
        self.enumerate()
            .max_by(|x, y| Ord::cmp(&key(&x.1), &key(&y.1)))
            .map(|x| x.0)
    }

    /// Return the position of the maximum element in the iterator, as
    /// determined by the specified comparison function.
    ///
    /// If several elements are equally maximum, the position of the
    /// last of them is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().position_max_by(|x, y| x.cmp(y)), None);
    ///
    /// let a = [-3_i32, 0, 1, 5, -10];
    /// assert_eq!(a.iter().position_max_by(|x, y| x.cmp(y)), Some(3));
    ///
    /// let a = [1_i32, 1, -1, -1];
    /// assert_eq!(a.iter().position_max_by(|x, y| x.cmp(y)), Some(1));
    /// ```
    fn position_max_by<F>(self, mut compare: F) -> Option<usize>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        self.enumerate()
            .max_by(|x, y| compare(&x.1, &y.1))
            .map(|x| x.0)
    }

    /// Return the position of the minimum element in the iterator.
    ///
    /// If several elements are equally minimum, the position of the
    /// first of them is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().position_min(), None);
    ///
    /// let a = [-3, 0, 1, 5, -10];
    /// assert_eq!(a.iter().position_min(), Some(4));
    ///
    /// let a = [1, 1, -1, -1];
    /// assert_eq!(a.iter().position_min(), Some(2));
    /// ```
    fn position_min(self) -> Option<usize>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        self.enumerate()
            .min_by(|x, y| Ord::cmp(&x.1, &y.1))
            .map(|x| x.0)
    }

    /// Return the position of the minimum element in the iterator, as
    /// determined by the specified function.
    ///
    /// If several elements are equally minimum, the position of the
    /// first of them is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().position_min_by_key(|x| x.abs()), None);
    ///
    /// let a = [-3_i32, 0, 1, 5, -10];
    /// assert_eq!(a.iter().position_min_by_key(|x| x.abs()), Some(1));
    ///
    /// let a = [1_i32, 1, -1, -1];
    /// assert_eq!(a.iter().position_min_by_key(|x| x.abs()), Some(0));
    /// ```
    fn position_min_by_key<K, F>(self, mut key: F) -> Option<usize>
    where
        Self: Sized,
        K: Ord,
        F: FnMut(&Self::Item) -> K,
    {
        self.enumerate()
            .min_by(|x, y| Ord::cmp(&key(&x.1), &key(&y.1)))
            .map(|x| x.0)
    }

    /// Return the position of the minimum element in the iterator, as
    /// determined by the specified comparison function.
    ///
    /// If several elements are equally minimum, the position of the
    /// first of them is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().position_min_by(|x, y| x.cmp(y)), None);
    ///
    /// let a = [-3_i32, 0, 1, 5, -10];
    /// assert_eq!(a.iter().position_min_by(|x, y| x.cmp(y)), Some(4));
    ///
    /// let a = [1_i32, 1, -1, -1];
    /// assert_eq!(a.iter().position_min_by(|x, y| x.cmp(y)), Some(2));
    /// ```
    fn position_min_by<F>(self, mut compare: F) -> Option<usize>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        self.enumerate()
            .min_by(|x, y| compare(&x.1, &y.1))
            .map(|x| x.0)
    }

    /// Return the positions of the minimum and maximum elements in
    /// the iterator.
    ///
    /// The return type [`MinMaxResult`] is an enum of three variants:
    ///
    /// - `NoElements` if the iterator is empty.
    /// - `OneElement(xpos)` if the iterator has exactly one element.
    /// - `MinMax(xpos, ypos)` is returned otherwise, where the
    ///    element at `xpos` ≤ the element at `ypos`. While the
    ///    referenced elements themselves may be equal, `xpos` cannot
    ///    be equal to `ypos`.
    ///
    /// On an iterator of length `n`, `position_minmax` does `1.5 * n`
    /// comparisons, and so is faster than calling `position_min` and
    /// `position_max` separately which does `2 * n` comparisons.
    ///
    /// For the minimum, if several elements are equally minimum, the
    /// position of the first of them is returned. For the maximum, if
    /// several elements are equally maximum, the position of the last
    /// of them is returned.
    ///
    /// The elements can be floats but no particular result is
    /// guaranteed if an element is NaN.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    /// use itertools::MinMaxResult::{NoElements, OneElement, MinMax};
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().position_minmax(), NoElements);
    ///
    /// let a = [10];
    /// assert_eq!(a.iter().position_minmax(), OneElement(0));
    ///
    /// let a = [-3, 0, 1, 5, -10];
    /// assert_eq!(a.iter().position_minmax(), MinMax(4, 3));
    ///
    /// let a = [1, 1, -1, -1];
    /// assert_eq!(a.iter().position_minmax(), MinMax(2, 1));
    /// ```
    fn position_minmax(self) -> MinMaxResult<usize>
    where
        Self: Sized,
        Self::Item: PartialOrd,
    {
        use crate::MinMaxResult::{MinMax, NoElements, OneElement};
        match minmax::minmax_impl(self.enumerate(), |_| (), |x, y, _, _| x.1 < y.1) {
            NoElements => NoElements,
            OneElement(x) => OneElement(x.0),
            MinMax(x, y) => MinMax(x.0, y.0),
        }
    }

    /// Return the postions of the minimum and maximum elements of an
    /// iterator, as determined by the specified function.
    ///
    /// The return value is a variant of [`MinMaxResult`] like for
    /// [`position_minmax`].
    ///
    /// For the minimum, if several elements are equally minimum, the
    /// position of the first of them is returned. For the maximum, if
    /// several elements are equally maximum, the position of the last
    /// of them is returned.
    ///
    /// The keys can be floats but no particular result is guaranteed
    /// if a key is NaN.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    /// use itertools::MinMaxResult::{NoElements, OneElement, MinMax};
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().position_minmax_by_key(|x| x.abs()), NoElements);
    ///
    /// let a = [10_i32];
    /// assert_eq!(a.iter().position_minmax_by_key(|x| x.abs()), OneElement(0));
    ///
    /// let a = [-3_i32, 0, 1, 5, -10];
    /// assert_eq!(a.iter().position_minmax_by_key(|x| x.abs()), MinMax(1, 4));
    ///
    /// let a = [1_i32, 1, -1, -1];
    /// assert_eq!(a.iter().position_minmax_by_key(|x| x.abs()), MinMax(0, 3));
    /// ```
    ///
    /// [`position_minmax`]: Self::position_minmax
    fn position_minmax_by_key<K, F>(self, mut key: F) -> MinMaxResult<usize>
    where
        Self: Sized,
        K: PartialOrd,
        F: FnMut(&Self::Item) -> K,
    {
        use crate::MinMaxResult::{MinMax, NoElements, OneElement};
        match self.enumerate().minmax_by_key(|e| key(&e.1)) {
            NoElements => NoElements,
            OneElement(x) => OneElement(x.0),
            MinMax(x, y) => MinMax(x.0, y.0),
        }
    }

    /// Return the postions of the minimum and maximum elements of an
    /// iterator, as determined by the specified comparison function.
    ///
    /// The return value is a variant of [`MinMaxResult`] like for
    /// [`position_minmax`].
    ///
    /// For the minimum, if several elements are equally minimum, the
    /// position of the first of them is returned. For the maximum, if
    /// several elements are equally maximum, the position of the last
    /// of them is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use itertools::Itertools;
    /// use itertools::MinMaxResult::{NoElements, OneElement, MinMax};
    ///
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.iter().position_minmax_by(|x, y| x.cmp(y)), NoElements);
    ///
    /// let a = [10_i32];
    /// assert_eq!(a.iter().position_minmax_by(|x, y| x.cmp(y)), OneElement(0));
    ///
    /// let a = [-3_i32, 0, 1, 5, -10];
    /// assert_eq!(a.iter().position_minmax_by(|x, y| x.cmp(y)), MinMax(4, 3));
    ///
    /// let a = [1_i32, 1, -1, -1];
    /// assert_eq!(a.iter().position_minmax_by(|x, y| x.cmp(y)), MinMax(2, 1));
    /// ```
    ///
    /// [`position_minmax`]: Self::position_minmax
    fn position_minmax_by<F>(self, mut compare: F) -> MinMaxResult<usize>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        use crate::MinMaxResult::{MinMax, NoElements, OneElement};
        match self.enumerate().minmax_by(|x, y| compare(&x.1, &y.1)) {
            NoElements => NoElements,
            OneElement(x) => OneElement(x.0),
            MinMax(x, y) => MinMax(x.0, y.0),
        }
    }

    /// If the iterator yields exactly one element, that element will be returned, otherwise
    /// an error will be returned containing an iterator that has the same output as the input
    /// iterator.
    ///
    /// This provides an additional layer of validation over just calling `Iterator::next()`.
    /// If your assumption that there should only be one element yielded is false this provides
    /// the opportunity to detect and handle that, preventing errors at a distance.
    ///
    /// # Examples
    /// ```
    /// use itertools::Itertools;
    ///
    /// assert_eq!((0..10).filter(|&x| x == 2).exactly_one().unwrap(), 2);
    /// assert!((0..10).filter(|&x| x > 1 && x < 4).exactly_one().unwrap_err().eq(2..4));
    /// assert!((0..10).filter(|&x| x > 1 && x < 5).exactly_one().unwrap_err().eq(2..5));
    /// assert!((0..10).filter(|&_| false).exactly_one().unwrap_err().eq(0..0));
    /// ```
    fn exactly_one(mut self) -> Result<Self::Item, ExactlyOneError<Self>>
    where
        Self: Sized,
    {
        match self.next() {
            Some(first) => match self.next() {
                Some(second) => Err(ExactlyOneError::new(
                    Some(Either::Left([first, second])),
                    self,
                )),
                None => Ok(first),
            },
            None => Err(ExactlyOneError::new(None, self)),
        }
    }

    /// If the iterator yields no elements, `Ok(None)` will be returned. If the iterator yields
    /// exactly one element, that element will be returned, otherwise an error will be returned
    /// containing an iterator that has the same output as the input iterator.
    ///
    /// This provides an additional layer of validation over just calling `Iterator::next()`.
    /// If your assumption that there should be at most one element yielded is false this provides
    /// the opportunity to detect and handle that, preventing errors at a distance.
    ///
    /// # Examples
    /// ```
    /// use itertools::Itertools;
    ///
    /// assert_eq!((0..10).filter(|&x| x == 2).at_most_one().unwrap(), Some(2));
    /// assert!((0..10).filter(|&x| x > 1 && x < 4).at_most_one().unwrap_err().eq(2..4));
    /// assert!((0..10).filter(|&x| x > 1 && x < 5).at_most_one().unwrap_err().eq(2..5));
    /// assert_eq!((0..10).filter(|&_| false).at_most_one().unwrap(), None);
    /// ```
    fn at_most_one(mut self) -> Result<Option<Self::Item>, ExactlyOneError<Self>>
    where
        Self: Sized,
    {
        match self.next() {
            Some(first) => match self.next() {
                Some(second) => Err(ExactlyOneError::new(
                    Some(Either::Left([first, second])),
                    self,
                )),
                None => Ok(Some(first)),
            },
            None => Ok(None),
        }
    }

    /// An iterator adaptor that allows the user to peek at multiple `.next()`
    /// values without advancing the base iterator.
    ///
    /// # Examples
    /// ```
    /// use itertools::Itertools;
    ///
    /// let mut iter = (0..10).multipeek();
    /// assert_eq!(iter.peek(), Some(&0));
    /// assert_eq!(iter.peek(), Some(&1));
    /// assert_eq!(iter.peek(), Some(&2));
    /// assert_eq!(iter.next(), Some(0));
    /// assert_eq!(iter.peek(), Some(&1));
    /// ```
    #[cfg(feature = "use_alloc")]
    fn multipeek(self) -> MultiPeek<Self>
    where
        Self: Sized,
    {
        multipeek_impl::multipeek(self)
    }

    /// Collect the items in this iterator and return a `HashMap` which
    /// contains each item that appears in the iterator and the number
    /// of times it appears.
    ///
    /// # Examples
    /// ```
    /// # use itertools::Itertools;
    /// let counts = [1, 1, 1, 3, 3, 5].iter().counts();
    /// assert_eq!(counts[&1], 3);
    /// assert_eq!(counts[&3], 2);
    /// assert_eq!(counts[&5], 1);
    /// assert_eq!(counts.get(&0), None);
    /// ```
    #[cfg(feature = "use_std")]
    fn counts(self) -> HashMap<Self::Item, usize>
    where
        Self: Sized,
        Self::Item: Eq + Hash,
    {
        let mut counts = HashMap::new();
        self.for_each(|item| *counts.entry(item).or_default() += 1);
        counts
    }

    /// Collect the items in this iterator and return a `HashMap` which
    /// contains each item that appears in the iterator and the number
    /// of times it appears,
    /// determining identity using a keying function.
    ///
    /// ```
    /// # use itertools::Itertools;
    /// struct Character {
    ///   first_name: &'static str,
    ///   # #[allow(dead_code)]
    ///   last_name:  &'static str,
    /// }
    ///
    /// let characters =
    ///     vec![
    ///         Character { first_name: "Amy",   last_name: "Pond"      },
    ///         Character { first_name: "Amy",   last_name: "Wong"      },
    ///         Character { first_name: "Amy",   last_name: "Santiago"  },
    ///         Character { first_name: "James", last_name: "Bond"      },
    ///         Character { first_name: "James", last_name: "Sullivan"  },
    ///         Character { first_name: "James", last_name: "Norington" },
    ///         Character { first_name: "James", last_name: "Kirk"      },
    ///     ];
    ///
    /// let first_name_frequency =
    ///     characters
    ///         .into_iter()
    ///         .counts_by(|c| c.first_name);
    ///
    /// assert_eq!(first_name_frequency["Amy"], 3);
    /// assert_eq!(first_name_frequency["James"], 4);
    /// assert_eq!(first_name_frequency.contains_key("Asha"), false);
    /// ```
    #[cfg(feature = "use_std")]
    fn counts_by<K, F>(self, f: F) -> HashMap<K, usize>
    where
        Self: Sized,
        K: Eq + Hash,
        F: FnMut(Self::Item) -> K,
    {
        self.map(f).counts()
    }

    /// Converts an iterator of tuples into a tuple of containers.
    ///
    /// It consumes an entire iterator of n-ary tuples, producing `n` collections, one for each
    /// column.
    ///
    /// This function is, in some sense, the opposite of [`multizip`].
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let inputs = vec![(1, 2, 3), (4, 5, 6), (7, 8, 9)];
    ///
    /// let (a, b, c): (Vec<_>, Vec<_>, Vec<_>) = inputs
    ///     .into_iter()
    ///     .multiunzip();
    ///
    /// assert_eq!(a, vec![1, 4, 7]);
    /// assert_eq!(b, vec![2, 5, 8]);
    /// assert_eq!(c, vec![3, 6, 9]);
    /// ```
    fn multiunzip<FromI>(self) -> FromI
    where
        Self: Sized + MultiUnzip<FromI>,
    {
        MultiUnzip::multiunzip(self)
    }

    /// Returns the length of the iterator if one exists.
    /// Otherwise return `self.size_hint()`.
    ///
    /// Fallible [`ExactSizeIterator::len`].
    ///
    /// Inherits guarantees and restrictions from [`Iterator::size_hint`].
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// assert_eq!([0; 10].iter().try_len(), Ok(10));
    /// assert_eq!((10..15).try_len(), Ok(5));
    /// assert_eq!((15..10).try_len(), Ok(0));
    /// assert_eq!((10..).try_len(), Err((usize::MAX, None)));
    /// assert_eq!((10..15).filter(|x| x % 2 == 0).try_len(), Err((0, Some(5))));
    /// ```
    fn try_len(&self) -> Result<usize, size_hint::SizeHint> {
        let sh = self.size_hint();
        match sh {
            (lo, Some(hi)) if lo == hi => Ok(lo),
            _ => Err(sh),
        }
    }
}

impl<T> Itertools for T where T: Iterator + ?Sized {}

/// Return `true` if both iterables produce equal sequences
/// (elements pairwise equal and sequences of the same length),
/// `false` otherwise.
///
/// [`IntoIterator`] enabled version of [`Iterator::eq`].
///
/// ```
/// assert!(itertools::equal(vec![1, 2, 3], 1..4));
/// assert!(!itertools::equal(&[0, 0], &[0, 0, 0]));
/// ```
pub fn equal<I, J>(a: I, b: J) -> bool
where
    I: IntoIterator,
    J: IntoIterator,
    I::Item: PartialEq<J::Item>,
{
    a.into_iter().eq(b)
}

/// Assert that two iterables produce equal sequences, with the same
/// semantics as [`equal(a, b)`](equal).
///
/// **Panics** on assertion failure with a message that shows the
/// two different elements and the iteration index.
///
/// ```should_panic
/// # use itertools::assert_equal;
/// assert_equal("exceed".split('c'), "excess".split('c'));
/// // ^PANIC: panicked at 'Failed assertion Some("eed") == Some("ess") for iteration 1'.
/// ```
#[track_caller]
pub fn assert_equal<I, J>(a: I, b: J)
where
    I: IntoIterator,
    J: IntoIterator,
    I::Item: fmt::Debug + PartialEq<J::Item>,
    J::Item: fmt::Debug,
{
    let mut ia = a.into_iter();
    let mut ib = b.into_iter();
    let mut i: usize = 0;
    loop {
        match (ia.next(), ib.next()) {
            (None, None) => return,
            (a, b) => {
                let equal = match (&a, &b) {
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                };
                assert!(
                    equal,
                    "Failed assertion {a:?} == {b:?} for iteration {i}",
                    i = i,
                    a = a,
                    b = b
                );
                i += 1;
            }
        }
    }
}

/// Partition a sequence using predicate `pred` so that elements
/// that map to `true` are placed before elements which map to `false`.
///
/// The order within the partitions is arbitrary.
///
/// Return the index of the split point.
///
/// ```
/// use itertools::partition;
///
/// # // use repeated numbers to not promise any ordering
/// let mut data = [7, 1, 1, 7, 1, 1, 7];
/// let split_index = partition(&mut data, |elt| *elt >= 3);
///
/// assert_eq!(data, [7, 7, 7, 1, 1, 1, 1]);
/// assert_eq!(split_index, 3);
/// ```
pub fn partition<'a, A: 'a, I, F>(iter: I, mut pred: F) -> usize
where
    I: IntoIterator<Item = &'a mut A>,
    I::IntoIter: DoubleEndedIterator,
    F: FnMut(&A) -> bool,
{
    let mut split_index = 0;
    let mut iter = iter.into_iter();
    while let Some(front) = iter.next() {
        if !pred(front) {
            match iter.rfind(|back| pred(back)) {
                Some(back) => std::mem::swap(front, back),
                None => break,
            }
        }
        split_index += 1;
    }
    split_index
}

/// An enum used for controlling the execution of `fold_while`.
///
/// See [`.fold_while()`](Itertools::fold_while) for more information.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FoldWhile<T> {
    /// Continue folding with this value
    Continue(T),
    /// Fold is complete and will return this value
    Done(T),
}

impl<T> FoldWhile<T> {
    /// Return the value in the continue or done.
    pub fn into_inner(self) -> T {
        match self {
            Self::Continue(x) | Self::Done(x) => x,
        }
    }

    /// Return true if `self` is `Done`, false if it is `Continue`.
    pub fn is_done(&self) -> bool {
        match *self {
            Self::Continue(_) => false,
            Self::Done(_) => true,
        }
    }
}
