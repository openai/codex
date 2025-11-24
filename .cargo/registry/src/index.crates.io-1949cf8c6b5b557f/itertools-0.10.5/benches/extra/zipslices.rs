use std::cmp;

// Note: There are different ways to implement ZipSlices.
// This version performed the best in benchmarks.
//
// I also implemented a version with three pointers (tptr, tend, uptr),
// that mimiced slice::Iter and only checked bounds by using tptr == tend,
// but that was inferior to this solution.

/// An iterator which iterates two slices simultaneously.
///
/// `ZipSlices` acts like a double-ended `.zip()` iterator.
///
/// It was intended to be more efficient than `.zip()`, and it was, then
/// rustc changed how it optimizes so it can not promise improved performance
/// at this time.
///
/// Note that elements past the end of the shortest of the two slices are ignored.
///
/// Iterator element type for `ZipSlices<T, U>` is `(T::Item, U::Item)`. For example,
/// for a `ZipSlices<&'a [A], &'b mut [B]>`, the element type is `(&'a A, &'b mut B)`.
#[derive(Clone)]
pub struct ZipSlices<T, U> {
    t: T,
    u: U,
    len: usize,
    index: usize,
}

impl<'a, 'b, A, B> ZipSlices<&'a [A], &'b [B]> {
    /// Create a new `ZipSlices` from slices `a` and `b`.
    ///
    /// Act like a double-ended `.zip()` iterator, but more efficiently.
    ///
    /// Note that elements past the end of the shortest of the two slices are ignored.
    #[inline(always)]
    pub fn new(a: &'a [A], b: &'b [B]) -> Self {
        let minl = cmp::min(a.len(), b.len());
        ZipSlices {
            t: a,
            u: b,
            len: minl,
            index: 0,
        }
    }
}

impl<T, U> ZipSlices<T, U>
    where T: Slice,
          U: Slice
{
    /// Create a new `ZipSlices` from slices `a` and `b`.
    ///
    /// Act like a double-ended `.zip()` iterator, but more efficiently.
    ///
    /// Note that elements past the end of the shortest of the two slices are ignored.
    #[inline(always)]
    pub fn from_slices(a: T, b: U) -> Self {
        let minl = cmp::min(a.len(), b.len());
        ZipSlices {
            t: a,
            u: b,
            len: minl,
            index: 0,
        }
    }
}

impl<T, U> Iterator for ZipSlices<T, U>
    where T: Slice,
          U: Slice
{
    type Item = (T::Item, U::Item);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.index >= self.len {
                None
            } else {
                let i = self.index;
                self.index += 1;
                Some((
                    self.t.get_unchecked(i),
                    self.u.get_unchecked(i)))
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len - self.index;
        (len, Some(len))
    }
}

impl<T, U> DoubleEndedIterator for ZipSlices<T, U>
    where T: Slice,
          U: Slice
{
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.index >= self.len {
                None
            } else {
                self.len -= 1;
                let i = self.len;
                Some((
                    self.t.get_unchecked(i),
                    self.u.get_unchecked(i)))
            }
        }
    }
}

impl<T, U> ExactSizeIterator for ZipSlices<T, U>
    where T: Slice,
          U: Slice
{}

unsafe impl<T, U> Slice for ZipSlices<T, U>
    where T: Slice,
          U: Slice
{
    type Item = (T::Item, U::Item);

    fn len(&self) -> usize {
        self.len - self.index
    }

    unsafe fn get_unchecked(&mut self, i: usize) -> Self::Item {
        (self.t.get_unchecked(i),
         self.u.get_unchecked(i))
    }
}

/// A helper trait to let `ZipSlices` accept both `&[T]` and `&mut [T]`.
///
/// Unsafe trait because:
///
/// - Implementors must guarantee that `get_unchecked` is valid for all indices `0..len()`.
pub unsafe trait Slice {
    /// The type of a reference to the slice's elements
    type Item;
    #[doc(hidden)]
    fn len(&self) -> usize;
    #[doc(hidden)]
    unsafe fn get_unchecked(&mut self, i: usize) -> Self::Item;
}

unsafe impl<'a, T> Slice for &'a [T] {
    type Item = &'a T;
    #[inline(always)]
    fn len(&self) -> usize { (**self).len() }
    #[inline(always)]
    unsafe fn get_unchecked(&mut self, i: usize) -> &'a T {
        debug_assert!(i < self.len());
        (**self).get_unchecked(i)
    }
}

unsafe impl<'a, T> Slice for &'a mut [T] {
    type Item = &'a mut T;
    #[inline(always)]
    fn len(&self) -> usize { (**self).len() }
    #[inline(always)]
    unsafe fn get_unchecked(&mut self, i: usize) -> &'a mut T {
        debug_assert!(i < self.len());
        // override the lifetime constraints of &mut &'a mut [T]
        (*(*self as *mut [T])).get_unchecked_mut(i)
    }
}

#[test]
fn zipslices() {

    let xs = [1, 2, 3, 4, 5, 6];
    let ys = [1, 2, 3, 7];
    ::itertools::assert_equal(ZipSlices::new(&xs, &ys), xs.iter().zip(&ys));

    let xs = [1, 2, 3, 4, 5, 6];
    let mut ys = [0; 6];
    for (x, y) in ZipSlices::from_slices(&xs[..], &mut ys[..]) {
        *y = *x;
    }
    ::itertools::assert_equal(&xs, &ys);
}
