//! Iterators that are sources (produce elements from parameters,
//! not from another iterator).
#![allow(deprecated)]

use std::fmt;
use std::mem;

/// Creates a new unfold source with the specified closure as the "iterator
/// function" and an initial state to eventually pass to the closure
///
/// `unfold` is a general iterator builder: it has a mutable state value,
/// and a closure with access to the state that produces the next value.
///
/// This more or less equivalent to a regular struct with an [`Iterator`]
/// implementation, and is useful for one-off iterators.
///
/// ```
/// // an iterator that yields sequential Fibonacci numbers,
/// // and stops at the maximum representable value.
///
/// use itertools::unfold;
///
/// let mut fibonacci = unfold((1u32, 1u32), |(x1, x2)| {
///     // Attempt to get the next Fibonacci number
///     let next = x1.saturating_add(*x2);
///
///     // Shift left: ret <- x1 <- x2 <- next
///     let ret = *x1;
///     *x1 = *x2;
///     *x2 = next;
///
///     // If addition has saturated at the maximum, we are finished
///     if ret == *x1 && ret > 1 {
///         None
///     } else {
///         Some(ret)
///     }
/// });
///
/// itertools::assert_equal(fibonacci.by_ref().take(8),
///                         vec![1, 1, 2, 3, 5, 8, 13, 21]);
/// assert_eq!(fibonacci.last(), Some(2_971_215_073))
/// ```
#[deprecated(
    note = "Use [std::iter::from_fn](https://doc.rust-lang.org/std/iter/fn.from_fn.html) instead",
    since = "0.13.0"
)]
pub fn unfold<A, St, F>(initial_state: St, f: F) -> Unfold<St, F>
where
    F: FnMut(&mut St) -> Option<A>,
{
    Unfold {
        f,
        state: initial_state,
    }
}

impl<St, F> fmt::Debug for Unfold<St, F>
where
    St: fmt::Debug,
{
    debug_fmt_fields!(Unfold, state);
}

/// See [`unfold`](crate::unfold) for more information.
#[derive(Clone)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[deprecated(
    note = "Use [std::iter::FromFn](https://doc.rust-lang.org/std/iter/struct.FromFn.html) instead",
    since = "0.13.0"
)]
pub struct Unfold<St, F> {
    f: F,
    /// Internal state that will be passed to the closure on the next iteration
    pub state: St,
}

impl<A, St, F> Iterator for Unfold<St, F>
where
    F: FnMut(&mut St) -> Option<A>,
{
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        (self.f)(&mut self.state)
    }
}

/// An iterator that infinitely applies function to value and yields results.
///
/// This `struct` is created by the [`iterate()`](crate::iterate) function.
/// See its documentation for more.
#[derive(Clone)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Iterate<St, F> {
    state: St,
    f: F,
}

impl<St, F> fmt::Debug for Iterate<St, F>
where
    St: fmt::Debug,
{
    debug_fmt_fields!(Iterate, state);
}

impl<St, F> Iterator for Iterate<St, F>
where
    F: FnMut(&St) -> St,
{
    type Item = St;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let next_state = (self.f)(&self.state);
        Some(mem::replace(&mut self.state, next_state))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

/// Creates a new iterator that infinitely applies function to value and yields results.
///
/// ```
/// use itertools::iterate;
///
/// itertools::assert_equal(iterate(1, |i| i % 3 + 1).take(5), vec![1, 2, 3, 1, 2]);
/// ```
///
/// **Panics** if compute the next value does.
///
/// ```should_panic
/// # use itertools::iterate;
/// let mut it = iterate(25u32, |x| x - 10).take_while(|&x| x > 10);
/// assert_eq!(it.next(), Some(25)); // `Iterate` holds 15.
/// assert_eq!(it.next(), Some(15)); // `Iterate` holds 5.
/// it.next(); // `5 - 10` overflows.
/// ```
///
/// You can alternatively use [`core::iter::successors`] as it better describes a finite iterator.
pub fn iterate<St, F>(initial_value: St, f: F) -> Iterate<St, F>
where
    F: FnMut(&St) -> St,
{
    Iterate {
        state: initial_value,
        f,
    }
}
