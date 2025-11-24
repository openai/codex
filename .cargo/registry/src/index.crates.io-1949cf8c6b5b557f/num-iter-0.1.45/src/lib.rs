// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! External iterators for generic mathematics
//!
//! ## Compatibility
//!
//! The `num-iter` crate is tested for rustc 1.31 and greater.

#![doc(html_root_url = "https://docs.rs/num-iter/0.1")]
#![no_std]

use core::ops::{Add, Bound, RangeBounds, Sub};
use core::usize;
use num_integer::Integer;
use num_traits::{CheckedAdd, One, ToPrimitive, Zero};

/// An iterator over the range [start, stop)
#[derive(Clone)]
pub struct Range<A> {
    state: A,
    stop: A,
    one: A,
}

/// Returns an iterator over the given range [start, stop) (that is, starting
/// at start (inclusive), and ending at stop (exclusive)).
///
/// # Example
///
/// ```rust
/// let array = [0, 1, 2, 3, 4];
///
/// for i in num_iter::range(0, 5) {
///     println!("{}", i);
///     assert_eq!(i,  array[i]);
/// }
/// ```
#[inline]
pub fn range<A>(start: A, stop: A) -> Range<A>
where
    A: Add<A, Output = A> + PartialOrd + Clone + One,
{
    Range {
        state: start,
        stop,
        one: One::one(),
    }
}

#[inline]
fn unsigned<T: ToPrimitive>(x: &T) -> Option<u128> {
    match x.to_u128() {
        Some(u) => Some(u),
        None => Some(x.to_i128()? as u128),
    }
}

impl<A> RangeBounds<A> for Range<A> {
    fn start_bound(&self) -> Bound<&A> {
        Bound::Included(&self.state)
    }

    fn end_bound(&self) -> Bound<&A> {
        Bound::Excluded(&self.stop)
    }
}

// FIXME: rust-lang/rust#10414: Unfortunate type bound
impl<A> Iterator for Range<A>
where
    A: Add<A, Output = A> + PartialOrd + Clone + ToPrimitive,
{
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        if self.state < self.stop {
            let result = self.state.clone();
            self.state = self.state.clone() + self.one.clone();
            Some(result)
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // Check for empty ranges first.
        if self.state >= self.stop {
            return (0, Some(0));
        }

        // Try to cast both ends to the largest unsigned primitive.
        // Note that negative values will wrap to a large positive.
        if let Some(a) = unsigned(&self.state) {
            if let Some(b) = unsigned(&self.stop) {
                // We've lost signs, but we already know state < stop, so
                // a `wrapping_sub` will give the correct unsigned delta.
                return match b.wrapping_sub(a).to_usize() {
                    Some(len) => (len, Some(len)),
                    None => (usize::MAX, None),
                };
            }
        }

        // Standard fallback for unbounded/unrepresentable bounds
        (0, None)
    }
}

/// `Integer` is required to ensure the range will be the same regardless of
/// the direction it is consumed.
impl<A> DoubleEndedIterator for Range<A>
where
    A: Integer + Clone + ToPrimitive,
{
    #[inline]
    fn next_back(&mut self) -> Option<A> {
        if self.stop > self.state {
            self.stop.dec();
            Some(self.stop.clone())
        } else {
            None
        }
    }
}

/// An iterator over the range [start, stop]
#[derive(Clone)]
pub struct RangeInclusive<A> {
    range: Range<A>,
    done: bool,
}

/// Return an iterator over the range [start, stop]
#[inline]
pub fn range_inclusive<A>(start: A, stop: A) -> RangeInclusive<A>
where
    A: Add<A, Output = A> + PartialOrd + Clone + One,
{
    RangeInclusive {
        range: range(start, stop),
        done: false,
    }
}

impl<A> RangeBounds<A> for RangeInclusive<A> {
    fn start_bound(&self) -> Bound<&A> {
        Bound::Included(&self.range.state)
    }

    fn end_bound(&self) -> Bound<&A> {
        Bound::Included(&self.range.stop)
    }
}

impl<A> Iterator for RangeInclusive<A>
where
    A: Add<A, Output = A> + PartialOrd + Clone + ToPrimitive,
{
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        match self.range.next() {
            Some(x) => Some(x),
            None => {
                if !self.done && self.range.state == self.range.stop {
                    self.done = true;
                    Some(self.range.stop.clone())
                } else {
                    None
                }
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lo, hi) = self.range.size_hint();
        if self.done {
            (lo, hi)
        } else {
            let lo = lo.saturating_add(1);
            let hi = match hi {
                Some(x) => x.checked_add(1),
                None => None,
            };
            (lo, hi)
        }
    }
}

impl<A> DoubleEndedIterator for RangeInclusive<A>
where
    A: Sub<A, Output = A> + Integer + Clone + ToPrimitive,
{
    #[inline]
    fn next_back(&mut self) -> Option<A> {
        if self.range.stop > self.range.state {
            let result = self.range.stop.clone();
            self.range.stop.dec();
            Some(result)
        } else if !self.done && self.range.state == self.range.stop {
            self.done = true;
            Some(self.range.stop.clone())
        } else {
            None
        }
    }
}

/// An iterator over the range [start, stop) by `step`. It handles overflow by stopping.
#[derive(Clone)]
pub struct RangeStep<A> {
    state: A,
    stop: A,
    step: A,
    rev: bool,
}

/// Return an iterator over the range [start, stop) by `step`. It handles overflow by stopping.
#[inline]
pub fn range_step<A>(start: A, stop: A, step: A) -> RangeStep<A>
where
    A: CheckedAdd + PartialOrd + Clone + Zero,
{
    let rev = step < Zero::zero();
    RangeStep {
        state: start,
        stop,
        step,
        rev,
    }
}

impl<A> Iterator for RangeStep<A>
where
    A: CheckedAdd + PartialOrd + Clone,
{
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        if (self.rev && self.state > self.stop) || (!self.rev && self.state < self.stop) {
            let result = self.state.clone();
            match self.state.checked_add(&self.step) {
                Some(x) => self.state = x,
                None => self.state = self.stop.clone(),
            }
            Some(result)
        } else {
            None
        }
    }
}

/// An iterator over the range [start, stop] by `step`. It handles overflow by stopping.
#[derive(Clone)]
pub struct RangeStepInclusive<A> {
    state: A,
    stop: A,
    step: A,
    rev: bool,
    done: bool,
}

/// Return an iterator over the range [start, stop] by `step`. It handles overflow by stopping.
#[inline]
pub fn range_step_inclusive<A>(start: A, stop: A, step: A) -> RangeStepInclusive<A>
where
    A: CheckedAdd + PartialOrd + Clone + Zero,
{
    let rev = step < Zero::zero();
    RangeStepInclusive {
        state: start,
        stop,
        step,
        rev,
        done: false,
    }
}

impl<A> Iterator for RangeStepInclusive<A>
where
    A: CheckedAdd + PartialOrd + Clone + PartialEq,
{
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        if !self.done
            && ((self.rev && self.state >= self.stop) || (!self.rev && self.state <= self.stop))
        {
            let result = self.state.clone();
            match self.state.checked_add(&self.step) {
                Some(x) => self.state = x,
                None => self.done = true,
            }
            Some(result)
        } else {
            None
        }
    }
}

/// An iterator over the infinite range starting at `start`
#[derive(Clone)]
pub struct RangeFrom<A> {
    state: A,
    one: A,
}

/// Return an iterator over the infinite range starting at `start` and continuing forever.
///
/// *Note*: Currently, the `Iterator` implementation is not checked for overflow.
/// If you use a finite-sized integer type and the integer overflows,
/// it might panic in debug mode or wrap around in release mode.
/// **This behavior is not guaranteed and may change at any time.**
#[inline]
pub fn range_from<A>(start: A) -> RangeFrom<A>
where
    A: Add<A, Output = A> + Clone + One,
{
    RangeFrom {
        state: start,
        one: One::one(),
    }
}

impl<A> RangeBounds<A> for RangeFrom<A> {
    fn start_bound(&self) -> Bound<&A> {
        Bound::Included(&self.state)
    }

    fn end_bound(&self) -> Bound<&A> {
        Bound::Unbounded
    }
}

impl<A> Iterator for RangeFrom<A>
where
    A: Add<A, Output = A> + Clone,
{
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        let result = self.state.clone();
        self.state = self.state.clone() + self.one.clone();
        Some(result)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

/// An iterator over the infinite range starting at `start` by `step`
#[derive(Clone)]
pub struct RangeStepFrom<A> {
    state: A,
    step: A,
}

/// Return an iterator over the infinite range starting at `start` and continuing forever by `step`.
///
/// *Note*: Currently, the `Iterator` implementation is not checked for overflow.
/// If you use a finite-sized integer type and the integer overflows,
/// it might panic in debug mode or wrap around in release mode.
/// **This behavior is not guaranteed and may change at any time.**
#[inline]
pub fn range_step_from<A>(start: A, step: A) -> RangeStepFrom<A>
where
    A: Add<A, Output = A> + Clone,
{
    RangeStepFrom { state: start, step }
}

impl<A> Iterator for RangeStepFrom<A>
where
    A: Add<A, Output = A> + Clone,
{
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        let result = self.state.clone();
        self.state = self.state.clone() + self.step.clone();
        Some(result)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}

#[cfg(test)]
mod tests {
    use core::cmp::Ordering;
    use core::iter;
    use core::ops::{Add, Mul};
    use core::{isize, usize};
    use num_traits::{One, ToPrimitive};

    #[test]
    fn test_range() {
        /// A mock type to check Range when ToPrimitive returns None
        struct Foo;

        impl ToPrimitive for Foo {
            fn to_i64(&self) -> Option<i64> {
                None
            }
            fn to_u64(&self) -> Option<u64> {
                None
            }
        }

        impl Add<Foo> for Foo {
            type Output = Foo;

            fn add(self, _: Foo) -> Foo {
                Foo
            }
        }

        impl PartialEq for Foo {
            fn eq(&self, _: &Foo) -> bool {
                true
            }
        }

        impl PartialOrd for Foo {
            fn partial_cmp(&self, _: &Foo) -> Option<Ordering> {
                None
            }
        }

        impl Clone for Foo {
            fn clone(&self) -> Foo {
                Foo
            }
        }

        impl Mul<Foo> for Foo {
            type Output = Foo;

            fn mul(self, _: Foo) -> Foo {
                Foo
            }
        }

        impl One for Foo {
            fn one() -> Foo {
                Foo
            }
        }

        assert!(super::range(0, 5).eq([0, 1, 2, 3, 4].iter().cloned()));
        assert!(super::range(-10, -1).eq([-10, -9, -8, -7, -6, -5, -4, -3, -2].iter().cloned()));
        assert!(super::range(0, 5).rev().eq([4, 3, 2, 1, 0].iter().cloned()));
        assert_eq!(super::range(200, -5).count(), 0);
        assert_eq!(super::range(200, -5).rev().count(), 0);
        assert_eq!(super::range(200, 200).count(), 0);
        assert_eq!(super::range(200, 200).rev().count(), 0);

        assert_eq!(super::range(0, 100).size_hint(), (100, Some(100)));
        // this test is only meaningful when sizeof usize < sizeof u64
        assert_eq!(
            super::range(usize::MAX - 1, usize::MAX).size_hint(),
            (1, Some(1))
        );
        assert_eq!(super::range(-10, -1).size_hint(), (9, Some(9)));
        assert_eq!(
            super::range(isize::MIN, isize::MAX).size_hint(),
            (usize::MAX, Some(usize::MAX))
        );
    }

    #[test]
    fn test_range_128() {
        use core::{i128, u128};

        assert!(super::range(0i128, 5).eq([0, 1, 2, 3, 4].iter().cloned()));
        assert!(super::range(-10i128, -1).eq([-10, -9, -8, -7, -6, -5, -4, -3, -2].iter().cloned()));
        assert!(super::range(0u128, 5)
            .rev()
            .eq([4, 3, 2, 1, 0].iter().cloned()));

        assert_eq!(
            super::range(i128::MIN, i128::MIN + 1).size_hint(),
            (1, Some(1))
        );
        assert_eq!(
            super::range(i128::MAX - 1, i128::MAX).size_hint(),
            (1, Some(1))
        );
        assert_eq!(
            super::range(i128::MIN, i128::MAX).size_hint(),
            (usize::MAX, None)
        );

        assert_eq!(
            super::range(u128::MAX - 1, u128::MAX).size_hint(),
            (1, Some(1))
        );
        assert_eq!(
            super::range(0, usize::MAX as u128).size_hint(),
            (usize::MAX, Some(usize::MAX))
        );
        assert_eq!(
            super::range(0, usize::MAX as u128 + 1).size_hint(),
            (usize::MAX, None)
        );
        assert_eq!(super::range(0, i128::MAX).size_hint(), (usize::MAX, None));
    }

    #[test]
    fn test_range_inclusive() {
        assert!(super::range_inclusive(0, 5).eq([0, 1, 2, 3, 4, 5].iter().cloned()));
        assert!(super::range_inclusive(0, 5)
            .rev()
            .eq([5, 4, 3, 2, 1, 0].iter().cloned()));
        assert_eq!(super::range_inclusive(200, -5).count(), 0);
        assert_eq!(super::range_inclusive(200, -5).rev().count(), 0);
        assert!(super::range_inclusive(200, 200).eq(iter::once(200)));
        assert!(super::range_inclusive(200, 200).rev().eq(iter::once(200)));
        assert_eq!(
            super::range_inclusive(isize::MIN, isize::MAX - 1).size_hint(),
            (usize::MAX, Some(usize::MAX))
        );
        assert_eq!(
            super::range_inclusive(isize::MIN, isize::MAX).size_hint(),
            (usize::MAX, None)
        );
    }

    #[test]
    fn test_range_inclusive_128() {
        use core::i128;

        assert!(super::range_inclusive(0u128, 5).eq([0, 1, 2, 3, 4, 5].iter().cloned()));
        assert!(super::range_inclusive(0u128, 5)
            .rev()
            .eq([5, 4, 3, 2, 1, 0].iter().cloned()));
        assert_eq!(super::range_inclusive(200i128, -5).count(), 0);
        assert_eq!(super::range_inclusive(200i128, -5).rev().count(), 0);
        assert!(super::range_inclusive(200u128, 200).eq(iter::once(200)));
        assert!(super::range_inclusive(200u128, 200)
            .rev()
            .eq(iter::once(200)));
        assert_eq!(
            super::range_inclusive(isize::MIN as i128, isize::MAX as i128 - 1).size_hint(),
            (usize::MAX, Some(usize::MAX))
        );
        assert_eq!(
            super::range_inclusive(isize::MIN as i128, isize::MAX as i128).size_hint(),
            (usize::MAX, None)
        );
        assert_eq!(
            super::range_inclusive(isize::MIN as i128, isize::MAX as i128 + 1).size_hint(),
            (usize::MAX, None)
        );
        assert_eq!(
            super::range_inclusive(i128::MIN, i128::MAX).size_hint(),
            (usize::MAX, None)
        );
    }

    #[test]
    fn test_range_step() {
        assert!(super::range_step(0, 20, 5).eq([0, 5, 10, 15].iter().cloned()));
        assert!(super::range_step(20, 0, -5).eq([20, 15, 10, 5].iter().cloned()));
        assert!(super::range_step(20, 0, -6).eq([20, 14, 8, 2].iter().cloned()));
        assert!(super::range_step(200u8, 255, 50).eq([200u8, 250].iter().cloned()));
        assert!(super::range_step(200, -5, 1).eq(iter::empty()));
        assert!(super::range_step(200, 200, 1).eq(iter::empty()));
    }

    #[test]
    fn test_range_step_128() {
        use core::u128::MAX as UMAX;

        assert!(super::range_step(0u128, 20, 5).eq([0, 5, 10, 15].iter().cloned()));
        assert!(super::range_step(20i128, 0, -5).eq([20, 15, 10, 5].iter().cloned()));
        assert!(super::range_step(20i128, 0, -6).eq([20, 14, 8, 2].iter().cloned()));
        assert!(super::range_step(UMAX - 55, UMAX, 50).eq([UMAX - 55, UMAX - 5].iter().cloned()));
        assert!(super::range_step(200i128, -5, 1).eq(iter::empty()));
        assert!(super::range_step(200i128, 200, 1).eq(iter::empty()));
    }

    #[test]
    fn test_range_step_inclusive() {
        assert!(super::range_step_inclusive(0, 20, 5).eq([0, 5, 10, 15, 20].iter().cloned()));
        assert!(super::range_step_inclusive(20, 0, -5).eq([20, 15, 10, 5, 0].iter().cloned()));
        assert!(super::range_step_inclusive(20, 0, -6).eq([20, 14, 8, 2].iter().cloned()));
        assert!(super::range_step_inclusive(200u8, 255, 50).eq([200u8, 250].iter().cloned()));
        assert!(super::range_step_inclusive(200, -5, 1).eq(iter::empty()));
        assert!(super::range_step_inclusive(200, 200, 1).eq(iter::once(200)));
    }

    #[test]
    fn test_range_step_inclusive_128() {
        use core::u128::MAX as UMAX;

        assert!(super::range_step_inclusive(0u128, 20, 5).eq([0, 5, 10, 15, 20].iter().cloned()));
        assert!(super::range_step_inclusive(20i128, 0, -5).eq([20, 15, 10, 5, 0].iter().cloned()));
        assert!(super::range_step_inclusive(20i128, 0, -6).eq([20, 14, 8, 2].iter().cloned()));
        assert!(super::range_step_inclusive(UMAX - 55, UMAX, 50)
            .eq([UMAX - 55, UMAX - 5].iter().cloned()));
        assert!(super::range_step_inclusive(200i128, -5, 1).eq(iter::empty()));
        assert!(super::range_step_inclusive(200i128, 200, 1).eq(iter::once(200)));
    }

    #[test]
    fn test_range_from() {
        assert!(super::range_from(10u8)
            .take(5)
            .eq([10, 11, 12, 13, 14].iter().cloned()));
        assert_eq!(super::range_from(10u8).size_hint(), (usize::MAX, None));
    }

    #[test]
    fn test_range_step_from() {
        assert!(super::range_step_from(10u8, 2u8)
            .take(5)
            .eq([10, 12, 14, 16, 18].iter().cloned()));
        assert_eq!(
            super::range_step_from(10u8, 2u8).size_hint(),
            (usize::MAX, None)
        );

        assert!(super::range_step_from(10u8, 1u8)
            .take(5)
            .eq([10, 11, 12, 13, 14].iter().cloned()));
        assert_eq!(
            super::range_step_from(10u8, 1u8).size_hint(),
            (usize::MAX, None)
        );

        assert!(super::range_step_from(10u8, 0u8)
            .take(5)
            .eq([10, 10, 10, 10, 10].iter().cloned()));
        assert_eq!(
            super::range_step_from(10u8, 0u8).size_hint(),
            (usize::MAX, None)
        );

        assert!(super::range_step_from(10i8, 2i8)
            .take(5)
            .eq([10, 12, 14, 16, 18].iter().cloned()));
        assert_eq!(
            super::range_step_from(10i8, 2i8).size_hint(),
            (usize::MAX, None)
        );

        assert!(super::range_step_from(10i8, 1i8)
            .take(5)
            .eq([10, 11, 12, 13, 14].iter().cloned()));
        assert_eq!(
            super::range_step_from(10i8, 1i8).size_hint(),
            (usize::MAX, None)
        );

        assert!(super::range_step_from(10i8, 0i8)
            .take(5)
            .eq([10, 10, 10, 10, 10].iter().cloned()));
        assert_eq!(
            super::range_step_from(10i8, 0i8).size_hint(),
            (usize::MAX, None)
        );

        assert!(super::range_step_from(10i8, -1i8)
            .take(5)
            .eq([10, 9, 8, 7, 6].iter().cloned()));
        assert_eq!(
            super::range_step_from(10i8, -1i8).size_hint(),
            (usize::MAX, None)
        );

        assert!(super::range_step_from(10i8, -2i8)
            .take(5)
            .eq([10, 8, 6, 4, 2].iter().cloned()));
        assert_eq!(
            super::range_step_from(10i8, -2i8).size_hint(),
            (usize::MAX, None)
        );
    }
}
