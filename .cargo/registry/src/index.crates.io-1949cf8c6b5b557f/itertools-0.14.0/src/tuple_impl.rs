//! Some iterator that produces tuples

use std::iter::Cycle;
use std::iter::Fuse;
use std::iter::FusedIterator;

use crate::size_hint;

// `HomogeneousTuple` is a public facade for `TupleCollect`, allowing
// tuple-related methods to be used by clients in generic contexts, while
// hiding the implementation details of `TupleCollect`.
// See https://github.com/rust-itertools/itertools/issues/387

/// Implemented for homogeneous tuples of size up to 12.
pub trait HomogeneousTuple: TupleCollect {}

impl<T: TupleCollect> HomogeneousTuple for T {}

/// An iterator over a incomplete tuple.
///
/// See [`.tuples()`](crate::Itertools::tuples) and
/// [`Tuples::into_buffer()`].
#[derive(Clone, Debug)]
pub struct TupleBuffer<T>
where
    T: HomogeneousTuple,
{
    cur: usize,
    buf: T::Buffer,
}

impl<T> TupleBuffer<T>
where
    T: HomogeneousTuple,
{
    fn new(buf: T::Buffer) -> Self {
        Self { cur: 0, buf }
    }
}

impl<T> Iterator for TupleBuffer<T>
where
    T: HomogeneousTuple,
{
    type Item = T::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let s = self.buf.as_mut();
        if let Some(ref mut item) = s.get_mut(self.cur) {
            self.cur += 1;
            item.take()
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let buffer = &self.buf.as_ref()[self.cur..];
        let len = if buffer.is_empty() {
            0
        } else {
            buffer
                .iter()
                .position(|x| x.is_none())
                .unwrap_or(buffer.len())
        };
        (len, Some(len))
    }
}

impl<T> ExactSizeIterator for TupleBuffer<T> where T: HomogeneousTuple {}

/// An iterator that groups the items in tuples of a specific size.
///
/// See [`.tuples()`](crate::Itertools::tuples) for more information.
#[derive(Clone, Debug)]
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Tuples<I, T>
where
    I: Iterator<Item = T::Item>,
    T: HomogeneousTuple,
{
    iter: Fuse<I>,
    buf: T::Buffer,
}

/// Create a new tuples iterator.
pub fn tuples<I, T>(iter: I) -> Tuples<I, T>
where
    I: Iterator<Item = T::Item>,
    T: HomogeneousTuple,
{
    Tuples {
        iter: iter.fuse(),
        buf: Default::default(),
    }
}

impl<I, T> Iterator for Tuples<I, T>
where
    I: Iterator<Item = T::Item>,
    T: HomogeneousTuple,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        T::collect_from_iter(&mut self.iter, &mut self.buf)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // The number of elts we've drawn from the underlying iterator, but have
        // not yet produced as a tuple.
        let buffered = T::buffer_len(&self.buf);
        // To that, we must add the size estimates of the underlying iterator.
        let (unbuffered_lo, unbuffered_hi) = self.iter.size_hint();
        // The total low estimate is the sum of the already-buffered elements,
        // plus the low estimate of remaining unbuffered elements, divided by
        // the tuple size.
        let total_lo = add_then_div(unbuffered_lo, buffered, T::num_items()).unwrap_or(usize::MAX);
        // And likewise for the total high estimate, but using the high estimate
        // of the remaining unbuffered elements.
        let total_hi = unbuffered_hi.and_then(|hi| add_then_div(hi, buffered, T::num_items()));
        (total_lo, total_hi)
    }
}

/// `(n + a) / d` avoiding overflow when possible, returns `None` if it overflows.
fn add_then_div(n: usize, a: usize, d: usize) -> Option<usize> {
    debug_assert_ne!(d, 0);
    (n / d).checked_add(a / d)?.checked_add((n % d + a % d) / d)
}

impl<I, T> ExactSizeIterator for Tuples<I, T>
where
    I: ExactSizeIterator<Item = T::Item>,
    T: HomogeneousTuple,
{
}

impl<I, T> Tuples<I, T>
where
    I: Iterator<Item = T::Item>,
    T: HomogeneousTuple,
{
    /// Return a buffer with the produced items that was not enough to be grouped in a tuple.
    ///
    /// ```
    /// use itertools::Itertools;
    ///
    /// let mut iter = (0..5).tuples();
    /// assert_eq!(Some((0, 1, 2)), iter.next());
    /// assert_eq!(None, iter.next());
    /// itertools::assert_equal(vec![3, 4], iter.into_buffer());
    /// ```
    pub fn into_buffer(self) -> TupleBuffer<T> {
        TupleBuffer::new(self.buf)
    }
}

/// An iterator over all contiguous windows that produces tuples of a specific size.
///
/// See [`.tuple_windows()`](crate::Itertools::tuple_windows) for more
/// information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Clone, Debug)]
pub struct TupleWindows<I, T>
where
    I: Iterator<Item = T::Item>,
    T: HomogeneousTuple,
{
    iter: I,
    last: Option<T>,
}

/// Create a new tuple windows iterator.
pub fn tuple_windows<I, T>(iter: I) -> TupleWindows<I, T>
where
    I: Iterator<Item = T::Item>,
    T: HomogeneousTuple,
    T::Item: Clone,
{
    TupleWindows { last: None, iter }
}

impl<I, T> Iterator for TupleWindows<I, T>
where
    I: Iterator<Item = T::Item>,
    T: HomogeneousTuple + Clone,
    T::Item: Clone,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if T::num_items() == 1 {
            return T::collect_from_iter_no_buf(&mut self.iter);
        }
        if let Some(new) = self.iter.next() {
            if let Some(ref mut last) = self.last {
                last.left_shift_push(new);
                Some(last.clone())
            } else {
                use std::iter::once;
                let iter = once(new).chain(&mut self.iter);
                self.last = T::collect_from_iter_no_buf(iter);
                self.last.clone()
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let mut sh = self.iter.size_hint();
        // Adjust the size hint at the beginning
        // OR when `num_items == 1` (but it does not change the size hint).
        if self.last.is_none() {
            sh = size_hint::sub_scalar(sh, T::num_items() - 1);
        }
        sh
    }
}

impl<I, T> ExactSizeIterator for TupleWindows<I, T>
where
    I: ExactSizeIterator<Item = T::Item>,
    T: HomogeneousTuple + Clone,
    T::Item: Clone,
{
}

impl<I, T> FusedIterator for TupleWindows<I, T>
where
    I: FusedIterator<Item = T::Item>,
    T: HomogeneousTuple + Clone,
    T::Item: Clone,
{
}

/// An iterator over all windows, wrapping back to the first elements when the
/// window would otherwise exceed the length of the iterator, producing tuples
/// of a specific size.
///
/// See [`.circular_tuple_windows()`](crate::Itertools::circular_tuple_windows) for more
/// information.
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
#[derive(Debug, Clone)]
pub struct CircularTupleWindows<I, T>
where
    I: Iterator<Item = T::Item> + Clone,
    T: TupleCollect + Clone,
{
    iter: TupleWindows<Cycle<I>, T>,
    len: usize,
}

pub fn circular_tuple_windows<I, T>(iter: I) -> CircularTupleWindows<I, T>
where
    I: Iterator<Item = T::Item> + Clone + ExactSizeIterator,
    T: TupleCollect + Clone,
    T::Item: Clone,
{
    let len = iter.len();
    let iter = tuple_windows(iter.cycle());

    CircularTupleWindows { iter, len }
}

impl<I, T> Iterator for CircularTupleWindows<I, T>
where
    I: Iterator<Item = T::Item> + Clone,
    T: TupleCollect + Clone,
    T::Item: Clone,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len != 0 {
            self.len -= 1;
            self.iter.next()
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<I, T> ExactSizeIterator for CircularTupleWindows<I, T>
where
    I: Iterator<Item = T::Item> + Clone,
    T: TupleCollect + Clone,
    T::Item: Clone,
{
}

impl<I, T> FusedIterator for CircularTupleWindows<I, T>
where
    I: Iterator<Item = T::Item> + Clone,
    T: TupleCollect + Clone,
    T::Item: Clone,
{
}

pub trait TupleCollect: Sized {
    type Item;
    type Buffer: Default + AsRef<[Option<Self::Item>]> + AsMut<[Option<Self::Item>]>;

    fn buffer_len(buf: &Self::Buffer) -> usize {
        let s = buf.as_ref();
        s.iter().position(Option::is_none).unwrap_or(s.len())
    }

    fn collect_from_iter<I>(iter: I, buf: &mut Self::Buffer) -> Option<Self>
    where
        I: IntoIterator<Item = Self::Item>;

    fn collect_from_iter_no_buf<I>(iter: I) -> Option<Self>
    where
        I: IntoIterator<Item = Self::Item>;

    fn num_items() -> usize;

    fn left_shift_push(&mut self, item: Self::Item);
}

macro_rules! rev_for_each_ident{
    ($m:ident, ) => {};
    ($m:ident, $i0:ident, $($i:ident,)*) => {
        rev_for_each_ident!($m, $($i,)*);
        $m!($i0);
    };
}

macro_rules! impl_tuple_collect {
    ($dummy:ident,) => {}; // stop
    ($dummy:ident, $($Y:ident,)*) => (
        impl_tuple_collect!($($Y,)*);
        impl<A> TupleCollect for ($(ignore_ident!($Y, A),)*) {
            type Item = A;
            type Buffer = [Option<A>; count_ident!($($Y)*) - 1];

            #[allow(unused_assignments, unused_mut)]
            fn collect_from_iter<I>(iter: I, buf: &mut Self::Buffer) -> Option<Self>
                where I: IntoIterator<Item = A>
            {
                let mut iter = iter.into_iter();
                $(
                    let mut $Y = None;
                )*

                loop {
                    $(
                        $Y = iter.next();
                        if $Y.is_none() {
                            break
                        }
                    )*
                    return Some(($($Y.unwrap()),*,))
                }

                let mut i = 0;
                let mut s = buf.as_mut();
                $(
                    if i < s.len() {
                        s[i] = $Y;
                        i += 1;
                    }
                )*
                return None;
            }

            fn collect_from_iter_no_buf<I>(iter: I) -> Option<Self>
                where I: IntoIterator<Item = A>
            {
                let mut iter = iter.into_iter();

                Some(($(
                    { let $Y = iter.next()?; $Y },
                )*))
            }

            fn num_items() -> usize {
                count_ident!($($Y)*)
            }

            fn left_shift_push(&mut self, mut item: A) {
                use std::mem::replace;

                let &mut ($(ref mut $Y),*,) = self;
                macro_rules! replace_item{($i:ident) => {
                    item = replace($i, item);
                }}
                rev_for_each_ident!(replace_item, $($Y,)*);
                drop(item);
            }
        }
    )
}
impl_tuple_collect!(dummy, a, b, c, d, e, f, g, h, i, j, k, l,);
