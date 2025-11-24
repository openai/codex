use super::*;

use core::{
  ops::{Bound, RangeBounds},
  slice,
};

/// Draining iterator for [`ArrayVec`]
///
/// See [`ArrayVec::drain`](ArrayVec::drain)
pub struct ArrayVecDrain<'a, T: 'a + Default> {
  iter: slice::IterMut<'a, T>,
}

impl<'a, T: 'a + Default> ArrayVecDrain<'a, T> {
  pub(crate) fn new<A, R>(arr: &'a mut ArrayVec<A>, range: R) -> Self
  where
    A: Array<Item = T>,
    R: RangeBounds<usize>,
  {
    let start = match range.start_bound() {
      Bound::Unbounded => 0,
      Bound::Included(&n) => n,
      Bound::Excluded(&n) => n.saturating_add(1),
    };
    let end = match range.end_bound() {
      Bound::Unbounded => arr.len(),
      Bound::Included(&n) => n.saturating_add(1),
      Bound::Excluded(&n) => n,
    };

    assert!(
      start <= end,
      "ArrayVec::drain> Illegal range, {} to {}",
      start,
      end
    );
    assert!(
      end <= arr.len(),
      "ArrayVec::drain> Range ends at {}, but length is only {}",
      end,
      arr.len()
    );

    let len = end - start;
    let to_rotate = &mut arr[start..];
    to_rotate.rotate_left(len);

    let oldlen = arr.len();
    let newlen = oldlen - len;
    arr.set_len(newlen);
    let slice = &mut arr.data.as_slice_mut()[newlen..oldlen];
    let iter = slice.iter_mut();
    Self { iter }
  }
}

impl<'a, T: 'a + Default> DoubleEndedIterator for ArrayVecDrain<'a, T> {
  #[inline]
  fn next_back(&mut self) -> Option<Self::Item> {
    self.iter.next_back().map(core::mem::take)
  }

  #[inline]
  fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
    self.iter.nth_back(n).map(core::mem::take)
  }
}

impl<'a, T: 'a + Default> Iterator for ArrayVecDrain<'a, T> {
  type Item = T;
  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    self.iter.next().map(core::mem::take)
  }
  #[inline]
  fn size_hint(&self) -> (usize, Option<usize>) {
    self.iter.size_hint()
  }
  #[inline]
  fn nth(&mut self, n: usize) -> Option<Self::Item> {
    self.iter.nth(n).map(core::mem::take)
  }
  #[inline]
  fn last(self) -> Option<Self::Item> {
    self.iter.last().map(core::mem::take)
  }
  #[inline]
  fn for_each<F>(self, f: F)
  where
    F: FnMut(Self::Item),
  {
    self.iter.map(core::mem::take).for_each(f)
  }
}

impl<'a, T: 'a + Default> FusedIterator for ArrayVecDrain<'a, T> {}
impl<'a, T: 'a + Default> ExactSizeIterator for ArrayVecDrain<'a, T> {}
/* No need to impl Drop! */
