//! [`Equivalent`] and [`Comparable`] are traits for key comparison in maps.
//!
//! These may be used in the implementation of maps where the lookup type `Q`
//! may be different than the stored key type `K`.
//!
//! * `Q: Equivalent<K>` checks for equality, similar to the `HashMap<K, V>`
//!   constraint `K: Borrow<Q>, Q: Eq`.
//! * `Q: Comparable<K>` checks the ordering, similar to the `BTreeMap<K, V>`
//!   constraint `K: Borrow<Q>, Q: Ord`.
//!
//! These traits are not used by the maps in the standard library, but they may
//! add more flexibility in third-party map implementations, especially in
//! situations where a strict `K: Borrow<Q>` relationship is not available.
//!
//! # Examples
//!
//! ```
//! use equivalent::*;
//! use std::cmp::Ordering;
//!
//! pub struct Pair<A, B>(pub A, pub B);
//!
//! impl<'a, A: ?Sized, B: ?Sized, C, D> Equivalent<(C, D)> for Pair<&'a A, &'a B>
//! where
//!     A: Equivalent<C>,
//!     B: Equivalent<D>,
//! {
//!     fn equivalent(&self, key: &(C, D)) -> bool {
//!         self.0.equivalent(&key.0) && self.1.equivalent(&key.1)
//!     }
//! }
//!
//! impl<'a, A: ?Sized, B: ?Sized, C, D> Comparable<(C, D)> for Pair<&'a A, &'a B>
//! where
//!     A: Comparable<C>,
//!     B: Comparable<D>,
//! {
//!     fn compare(&self, key: &(C, D)) -> Ordering {
//!         match self.0.compare(&key.0) {
//!             Ordering::Equal => self.1.compare(&key.1),
//!             not_equal => not_equal,
//!         }
//!     }
//! }
//!
//! fn main() {
//!     let key = (String::from("foo"), String::from("bar"));
//!     let q1 = Pair("foo", "bar");
//!     let q2 = Pair("boo", "bar");
//!     let q3 = Pair("foo", "baz");
//!
//!     assert!(q1.equivalent(&key));
//!     assert!(!q2.equivalent(&key));
//!     assert!(!q3.equivalent(&key));
//!
//!     assert_eq!(q1.compare(&key), Ordering::Equal);
//!     assert_eq!(q2.compare(&key), Ordering::Less);
//!     assert_eq!(q3.compare(&key), Ordering::Greater);
//! }
//! ```

#![no_std]

use core::borrow::Borrow;
use core::cmp::Ordering;

/// Key equivalence trait.
///
/// This trait allows hash table lookup to be customized. It has one blanket
/// implementation that uses the regular solution with `Borrow` and `Eq`, just
/// like `HashMap` does, so that you can pass `&str` to lookup into a map with
/// `String` keys and so on.
///
/// # Contract
///
/// The implementor **must** hash like `K`, if it is hashable.
pub trait Equivalent<K: ?Sized> {
    /// Compare self to `key` and return `true` if they are equal.
    fn equivalent(&self, key: &K) -> bool;
}

impl<Q: ?Sized, K: ?Sized> Equivalent<K> for Q
where
    Q: Eq,
    K: Borrow<Q>,
{
    #[inline]
    fn equivalent(&self, key: &K) -> bool {
        PartialEq::eq(self, key.borrow())
    }
}

/// Key ordering trait.
///
/// This trait allows ordered map lookup to be customized. It has one blanket
/// implementation that uses the regular solution with `Borrow` and `Ord`, just
/// like `BTreeMap` does, so that you can pass `&str` to lookup into a map with
/// `String` keys and so on.
pub trait Comparable<K: ?Sized>: Equivalent<K> {
    /// Compare self to `key` and return their ordering.
    fn compare(&self, key: &K) -> Ordering;
}

impl<Q: ?Sized, K: ?Sized> Comparable<K> for Q
where
    Q: Ord,
    K: Borrow<Q>,
{
    #[inline]
    fn compare(&self, key: &K) -> Ordering {
        Ord::cmp(self, key.borrow())
    }
}
