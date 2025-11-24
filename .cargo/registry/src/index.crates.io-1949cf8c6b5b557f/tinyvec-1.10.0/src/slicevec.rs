#![allow(unused_variables)]
#![allow(missing_docs)]

use super::*;

/// A slice-backed vector-like data structure.
///
/// This is a very similar concept to `ArrayVec`, but instead
/// of the backing memory being an owned array, the backing
/// memory is a unique-borrowed slice. You can thus create
/// one of these structures "around" some slice that you're
/// working with to make it easier to manipulate.
///
/// * Has a fixed capacity (the initial slice size).
/// * Has a variable length.
pub struct SliceVec<'s, T> {
  data: &'s mut [T],
  len: usize,
}

impl<'s, T> Default for SliceVec<'s, T> {
  #[inline(always)]
  fn default() -> Self {
    Self { data: &mut [], len: 0 }
  }
}

impl<'s, T> Deref for SliceVec<'s, T> {
  type Target = [T];
  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    &self.data[..self.len]
  }
}

impl<'s, T> DerefMut for SliceVec<'s, T> {
  #[inline(always)]
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.data[..self.len]
  }
}

impl<'s, T, I> Index<I> for SliceVec<'s, T>
where
  I: SliceIndex<[T]>,
{
  type Output = <I as SliceIndex<[T]>>::Output;
  #[inline(always)]
  fn index(&self, index: I) -> &Self::Output {
    &self.deref()[index]
  }
}

impl<'s, T, I> IndexMut<I> for SliceVec<'s, T>
where
  I: SliceIndex<[T]>,
{
  #[inline(always)]
  fn index_mut(&mut self, index: I) -> &mut Self::Output {
    &mut self.deref_mut()[index]
  }
}

impl<'s, T> SliceVec<'s, T> {
  #[inline]
  pub fn append(&mut self, other: &mut Self)
  where
    T: Default,
  {
    for item in other.drain(..) {
      self.push(item)
    }
  }

  /// A `*mut` pointer to the backing slice.
  ///
  /// ## Safety
  ///
  /// This pointer has provenance over the _entire_ backing slice.
  #[inline(always)]
  #[must_use]
  pub fn as_mut_ptr(&mut self) -> *mut T {
    self.data.as_mut_ptr()
  }

  /// Performs a `deref_mut`, into unique slice form.
  #[inline(always)]
  #[must_use]
  pub fn as_mut_slice(&mut self) -> &mut [T] {
    self.deref_mut()
  }

  /// A `*const` pointer to the backing slice.
  ///
  /// ## Safety
  ///
  /// This pointer has provenance over the _entire_ backing slice.
  #[inline(always)]
  #[must_use]
  pub fn as_ptr(&self) -> *const T {
    self.data.as_ptr()
  }

  /// Performs a `deref`, into shared slice form.
  #[inline(always)]
  #[must_use]
  pub fn as_slice(&self) -> &[T] {
    self.deref()
  }

  /// The capacity of the `SliceVec`.
  ///
  /// This the length of the initial backing slice.
  #[inline(always)]
  #[must_use]
  pub fn capacity(&self) -> usize {
    self.data.len()
  }

  /// Truncates the `SliceVec` down to length 0.
  #[inline(always)]
  pub fn clear(&mut self)
  where
    T: Default,
  {
    self.truncate(0)
  }

  /// Creates a draining iterator that removes the specified range in the vector
  /// and yields the removed items.
  ///
  /// ## Panics
  /// * If the start is greater than the end
  /// * If the end is past the edge of the vec.
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [6, 7, 8];
  /// let mut sv = SliceVec::from(&mut arr);
  /// let drained_values: ArrayVec<[i32; 4]> = sv.drain(1..).collect();
  /// assert_eq!(sv.as_slice(), &[6][..]);
  /// assert_eq!(drained_values.as_slice(), &[7, 8][..]);
  ///
  /// sv.drain(..);
  /// assert_eq!(sv.as_slice(), &[]);
  /// ```
  #[inline]
  pub fn drain<'p, R: RangeBounds<usize>>(
    &'p mut self, range: R,
  ) -> SliceVecDrain<'p, 's, T>
  where
    T: Default,
  {
    use core::ops::Bound;
    let start = match range.start_bound() {
      Bound::Included(x) => *x,
      Bound::Excluded(x) => x.saturating_add(1),
      Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
      Bound::Included(x) => x.saturating_add(1),
      Bound::Excluded(x) => *x,
      Bound::Unbounded => self.len,
    };
    assert!(
      start <= end,
      "SliceVec::drain> Illegal range, {} to {}",
      start,
      end
    );
    assert!(
      end <= self.len,
      "SliceVec::drain> Range ends at {} but length is only {}!",
      end,
      self.len
    );
    SliceVecDrain {
      parent: self,
      target_start: start,
      target_index: start,
      target_end: end,
    }
  }

  #[inline]
  pub fn extend_from_slice(&mut self, sli: &[T])
  where
    T: Clone,
  {
    if sli.is_empty() {
      return;
    }

    let new_len = self.len + sli.len();
    if new_len > self.capacity() {
      panic!(
        "SliceVec::extend_from_slice> total length {} exceeds capacity {}",
        new_len,
        self.capacity()
      )
    }

    let target = &mut self.data[self.len..new_len];
    target.clone_from_slice(sli);
    self.set_len(new_len);
  }

  /// Fill the vector until its capacity has been reached.
  ///
  /// Successively fills unused space in the spare slice of the vector with
  /// elements from the iterator. It then returns the remaining iterator
  /// without exhausting it. This also allows appending the head of an
  /// infinite iterator.
  ///
  /// This is an alternative to `Extend::extend` method for cases where the
  /// length of the iterator can not be checked. Since this vector can not
  /// reallocate to increase its capacity, it is unclear what to do with
  /// remaining elements in the iterator and the iterator itself. The
  /// interface also provides no way to communicate this to the caller.
  ///
  /// ## Panics
  /// * If the `next` method of the provided iterator panics.
  ///
  /// ## Example
  ///
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [7, 7, 7, 7];
  /// let mut sv = SliceVec::from_slice_len(&mut arr, 0);
  /// let mut to_inf = sv.fill(0..);
  /// assert_eq!(&sv[..], [0, 1, 2, 3]);
  /// assert_eq!(to_inf.next(), Some(4));
  /// ```
  #[inline]
  pub fn fill<I: IntoIterator<Item = T>>(&mut self, iter: I) -> I::IntoIter {
    let mut iter = iter.into_iter();
    for element in iter.by_ref().take(self.capacity() - self.len()) {
      self.push(element);
    }
    iter
  }

  /// Wraps up a slice and uses the given length as the initial length.
  ///
  /// If you want to simply use the full slice, use `from` instead.
  ///
  /// ## Panics
  ///
  /// * The length specified must be less than or equal to the capacity of the
  ///   slice.
  #[inline]
  #[must_use]
  #[allow(clippy::match_wild_err_arm)]
  pub fn from_slice_len(data: &'s mut [T], len: usize) -> Self {
    assert!(len <= data.len());
    Self { data, len }
  }

  /// Inserts an item at the position given, moving all following elements +1
  /// index.
  ///
  /// ## Panics
  /// * If `index` > `len`
  /// * If the capacity is exhausted
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [1, 2, 3, 0, 0];
  /// let mut sv = SliceVec::from_slice_len(&mut arr, 3);
  /// sv.insert(1, 4);
  /// assert_eq!(sv.as_slice(), &[1, 4, 2, 3]);
  /// sv.insert(4, 5);
  /// assert_eq!(sv.as_slice(), &[1, 4, 2, 3, 5]);
  /// ```
  #[inline]
  pub fn insert(&mut self, index: usize, item: T) {
    if index > self.len {
      panic!("SliceVec::insert> index {} is out of bounds {}", index, self.len);
    }

    // Try to push the element.
    self.push(item);
    // And move it into its place.
    self.as_mut_slice()[index..].rotate_right(1);
  }

  /// Checks if the length is 0.
  #[inline(always)]
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len == 0
  }

  /// The length of the `SliceVec` (in elements).
  #[inline(always)]
  #[must_use]
  pub fn len(&self) -> usize {
    self.len
  }

  /// Remove and return the last element of the vec, if there is one.
  ///
  /// ## Failure
  /// * If the vec is empty you get `None`.
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [1, 2];
  /// let mut sv = SliceVec::from(&mut arr);
  /// assert_eq!(sv.pop(), Some(2));
  /// assert_eq!(sv.pop(), Some(1));
  /// assert_eq!(sv.pop(), None);
  /// ```
  #[inline]
  pub fn pop(&mut self) -> Option<T>
  where
    T: Default,
  {
    if self.len > 0 {
      self.len -= 1;
      let out = core::mem::take(&mut self.data[self.len]);
      Some(out)
    } else {
      None
    }
  }

  /// Place an element onto the end of the vec.
  ///
  /// ## Panics
  /// * If the length of the vec would overflow the capacity.
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [0, 0];
  /// let mut sv = SliceVec::from_slice_len(&mut arr, 0);
  /// assert_eq!(&sv[..], []);
  /// sv.push(1);
  /// assert_eq!(&sv[..], [1]);
  /// sv.push(2);
  /// assert_eq!(&sv[..], [1, 2]);
  /// // sv.push(3); this would overflow the ArrayVec and panic!
  /// ```
  #[inline(always)]
  pub fn push(&mut self, val: T) {
    if self.len < self.capacity() {
      self.data[self.len] = val;
      self.len += 1;
    } else {
      panic!("SliceVec::push> capacity overflow")
    }
  }

  /// Removes the item at `index`, shifting all others down by one index.
  ///
  /// Returns the removed element.
  ///
  /// ## Panics
  ///
  /// * If the index is out of bounds.
  ///
  /// ## Example
  ///
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [1, 2, 3];
  /// let mut sv = SliceVec::from(&mut arr);
  /// assert_eq!(sv.remove(1), 2);
  /// assert_eq!(&sv[..], [1, 3]);
  /// ```
  #[inline]
  pub fn remove(&mut self, index: usize) -> T
  where
    T: Default,
  {
    let targets: &mut [T] = &mut self.deref_mut()[index..];
    let item = core::mem::take(&mut targets[0]);
    targets.rotate_left(1);
    self.len -= 1;
    item
  }

  /// As [`resize_with`](SliceVec::resize_with)
  /// and it clones the value as the closure.
  ///
  /// ## Example
  ///
  /// ```rust
  /// # use tinyvec::*;
  /// // bigger
  /// let mut arr = ["hello", "", "", "", ""];
  /// let mut sv = SliceVec::from_slice_len(&mut arr, 1);
  /// sv.resize(3, "world");
  /// assert_eq!(&sv[..], ["hello", "world", "world"]);
  ///
  /// // smaller
  /// let mut arr = ['a', 'b', 'c', 'd'];
  /// let mut sv = SliceVec::from(&mut arr);
  /// sv.resize(2, 'z');
  /// assert_eq!(&sv[..], ['a', 'b']);
  /// ```
  #[inline]
  pub fn resize(&mut self, new_len: usize, new_val: T)
  where
    T: Clone,
  {
    self.resize_with(new_len, || new_val.clone())
  }

  /// Resize the vec to the new length.
  ///
  /// * If it needs to be longer, it's filled with repeated calls to the
  ///   provided function.
  /// * If it needs to be shorter, it's truncated.
  ///   * If the type needs to drop the truncated slots are filled with calls to
  ///     the provided function.
  ///
  /// ## Example
  ///
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [1, 2, 3, 7, 7, 7, 7];
  /// let mut sv = SliceVec::from_slice_len(&mut arr, 3);
  /// sv.resize_with(5, Default::default);
  /// assert_eq!(&sv[..], [1, 2, 3, 0, 0]);
  ///
  /// let mut arr = [0, 0, 0, 0];
  /// let mut sv = SliceVec::from_slice_len(&mut arr, 0);
  /// let mut p = 1;
  /// sv.resize_with(4, || {
  ///   p *= 2;
  ///   p
  /// });
  /// assert_eq!(&sv[..], [2, 4, 8, 16]);
  /// ```
  #[inline]
  pub fn resize_with<F: FnMut() -> T>(&mut self, new_len: usize, mut f: F) {
    match new_len.checked_sub(self.len) {
      None => {
        if needs_drop::<T>() {
          while self.len() > new_len {
            self.len -= 1;
            self.data[self.len] = f();
          }
        } else {
          self.len = new_len;
        }
      }
      Some(new_elements) => {
        for _ in 0..new_elements {
          self.push(f());
        }
      }
    }
  }

  /// Walk the vec and keep only the elements that pass the predicate given.
  ///
  /// ## Example
  ///
  /// ```rust
  /// # use tinyvec::*;
  ///
  /// let mut arr = [1, 1, 2, 3, 3, 4];
  /// let mut sv = SliceVec::from(&mut arr);
  /// sv.retain(|&x| x % 2 == 0);
  /// assert_eq!(&sv[..], [2, 4]);
  /// ```
  #[inline]
  pub fn retain<F: FnMut(&T) -> bool>(&mut self, mut acceptable: F)
  where
    T: Default,
  {
    // Drop guard to contain exactly the remaining elements when the test
    // panics.
    struct JoinOnDrop<'vec, Item> {
      items: &'vec mut [Item],
      done_end: usize,
      // Start of tail relative to `done_end`.
      tail_start: usize,
    }

    impl<Item> Drop for JoinOnDrop<'_, Item> {
      fn drop(&mut self) {
        self.items[self.done_end..].rotate_left(self.tail_start);
      }
    }

    let mut rest = JoinOnDrop { items: self.data, done_end: 0, tail_start: 0 };

    for idx in 0..self.len {
      // Loop start invariant: idx = rest.done_end + rest.tail_start
      if !acceptable(&rest.items[idx]) {
        let _ = core::mem::take(&mut rest.items[idx]);
        self.len -= 1;
        rest.tail_start += 1;
      } else {
        rest.items.swap(rest.done_end, idx);
        rest.done_end += 1;
      }
    }
  }

  /// Forces the length of the vector to `new_len`.
  ///
  /// ## Panics
  /// * If `new_len` is greater than the vec's capacity.
  ///
  /// ## Safety
  /// * This is a fully safe operation! The inactive memory already counts as
  ///   "initialized" by Rust's rules.
  /// * Other than "the memory is initialized" there are no other guarantees
  ///   regarding what you find in the inactive portion of the vec.
  #[inline(always)]
  pub fn set_len(&mut self, new_len: usize) {
    if new_len > self.capacity() {
      // Note(Lokathor): Technically we don't have to panic here, and we could
      // just let some other call later on trigger a panic on accident when the
      // length is wrong. However, it's a lot easier to catch bugs when things
      // are more "fail-fast".
      panic!(
        "SliceVec::set_len> new length {} exceeds capacity {}",
        new_len,
        self.capacity()
      )
    } else {
      self.len = new_len;
    }
  }

  /// Splits the collection at the point given.
  ///
  /// * `[0, at)` stays in this vec (and this vec is now full).
  /// * `[at, len)` ends up in the new vec (with any spare capacity).
  ///
  /// ## Panics
  /// * if `at` > `self.len()`
  ///
  /// ## Example
  ///
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [1, 2, 3];
  /// let mut sv = SliceVec::from(&mut arr);
  /// let sv2 = sv.split_off(1);
  /// assert_eq!(&sv[..], [1]);
  /// assert_eq!(&sv2[..], [2, 3]);
  /// ```
  #[inline]
  pub fn split_off<'a>(&'a mut self, at: usize) -> SliceVec<'s, T> {
    let mut new = Self::default();
    let backing: &'s mut [T] = core::mem::take(&mut self.data);
    let (me, other) = backing.split_at_mut(at);
    new.len = self.len - at;
    new.data = other;
    self.len = me.len();
    self.data = me;
    new
  }

  /// Remove an element, swapping the end of the vec into its place.
  ///
  /// ## Panics
  /// * If the index is out of bounds.
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = ["foo", "bar", "quack", "zap"];
  /// let mut sv = SliceVec::from(&mut arr);
  ///
  /// assert_eq!(sv.swap_remove(1), "bar");
  /// assert_eq!(&sv[..], ["foo", "zap", "quack"]);
  ///
  /// assert_eq!(sv.swap_remove(0), "foo");
  /// assert_eq!(&sv[..], ["quack", "zap"]);
  /// ```
  #[inline]
  pub fn swap_remove(&mut self, index: usize) -> T
  where
    T: Default,
  {
    assert!(
      index < self.len,
      "SliceVec::swap_remove> index {} is out of bounds {}",
      index,
      self.len
    );
    if index == self.len - 1 {
      self.pop().unwrap()
    } else {
      let i = self.pop().unwrap();
      replace(&mut self[index], i)
    }
  }

  /// Reduces the vec's length to the given value.
  ///
  /// If the vec is already shorter than the input, nothing happens.
  #[inline]
  pub fn truncate(&mut self, new_len: usize)
  where
    T: Default,
  {
    if needs_drop::<T>() {
      while self.len > new_len {
        self.pop();
      }
    } else {
      self.len = self.len.min(new_len);
    }
  }

  /// Wraps a slice, using the given length as the starting length.
  ///
  /// If you want to use the whole length of the slice, you can just use the
  /// `From` impl.
  ///
  /// ## Failure
  ///
  /// If the given length is greater than the length of the slice you get
  /// `None`.
  #[inline]
  pub fn try_from_slice_len(data: &'s mut [T], len: usize) -> Option<Self> {
    if len <= data.len() {
      Some(Self { data, len })
    } else {
      None
    }
  }
}

#[cfg(feature = "grab_spare_slice")]
impl<'s, T> SliceVec<'s, T> {
  /// Obtain the shared slice of the array _after_ the active memory.
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [0; 4];
  /// let mut sv = SliceVec::from_slice_len(&mut arr, 0);
  /// assert_eq!(sv.grab_spare_slice().len(), 4);
  /// sv.push(10);
  /// sv.push(11);
  /// sv.push(12);
  /// sv.push(13);
  /// assert_eq!(sv.grab_spare_slice().len(), 0);
  /// ```
  #[must_use]
  #[inline(always)]
  pub fn grab_spare_slice(&self) -> &[T] {
    &self.data[self.len..]
  }

  /// Obtain the mutable slice of the array _after_ the active memory.
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [0; 4];
  /// let mut sv = SliceVec::from_slice_len(&mut arr, 0);
  /// assert_eq!(sv.grab_spare_slice_mut().len(), 4);
  /// sv.push(10);
  /// sv.push(11);
  /// assert_eq!(sv.grab_spare_slice_mut().len(), 2);
  /// ```
  #[inline(always)]
  pub fn grab_spare_slice_mut(&mut self) -> &mut [T] {
    &mut self.data[self.len..]
  }
}

impl<'s, T> From<&'s mut [T]> for SliceVec<'s, T> {
  /// Uses the full slice as the initial length.
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [0_i32; 2];
  /// let mut sv = SliceVec::from(&mut arr[..]);
  /// ```
  #[inline]
  fn from(data: &'s mut [T]) -> Self {
    let len = data.len();
    Self { data, len }
  }
}

impl<'s, T, A> From<&'s mut A> for SliceVec<'s, T>
where
  A: AsMut<[T]>,
{
  /// Calls `AsRef::as_mut` then uses the full slice as the initial length.
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut arr = [0, 0];
  /// let mut sv = SliceVec::from(&mut arr);
  /// ```
  #[inline]
  fn from(a: &'s mut A) -> Self {
    let data = a.as_mut();
    let len = data.len();
    Self { data, len }
  }
}

/// Draining iterator for [`SliceVec`]
///
/// See [`SliceVec::drain`](SliceVec::drain)
pub struct SliceVecDrain<'p, 's, T: Default> {
  parent: &'p mut SliceVec<'s, T>,
  target_start: usize,
  target_index: usize,
  target_end: usize,
}
impl<'p, 's, T: Default> Iterator for SliceVecDrain<'p, 's, T> {
  type Item = T;
  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.target_index != self.target_end {
      let out = core::mem::take(&mut self.parent[self.target_index]);
      self.target_index += 1;
      Some(out)
    } else {
      None
    }
  }
}
impl<'p, 's, T: Default> FusedIterator for SliceVecDrain<'p, 's, T> {}
impl<'p, 's, T: Default> Drop for SliceVecDrain<'p, 's, T> {
  #[inline]
  fn drop(&mut self) {
    // Changed because it was moving `self`, it's also more clear and the std
    // does the same
    self.for_each(drop);
    // Implementation very similar to [`SliceVec::remove`](SliceVec::remove)
    let count = self.target_end - self.target_start;
    let targets: &mut [T] = &mut self.parent.deref_mut()[self.target_start..];
    targets.rotate_left(count);
    self.parent.len -= count;
  }
}

impl<'s, T> AsMut<[T]> for SliceVec<'s, T> {
  #[inline(always)]
  fn as_mut(&mut self) -> &mut [T] {
    &mut *self
  }
}

impl<'s, T> AsRef<[T]> for SliceVec<'s, T> {
  #[inline(always)]
  fn as_ref(&self) -> &[T] {
    &*self
  }
}

impl<'s, T> Borrow<[T]> for SliceVec<'s, T> {
  #[inline(always)]
  fn borrow(&self) -> &[T] {
    &*self
  }
}

impl<'s, T> BorrowMut<[T]> for SliceVec<'s, T> {
  #[inline(always)]
  fn borrow_mut(&mut self) -> &mut [T] {
    &mut *self
  }
}

impl<'s, T> Extend<T> for SliceVec<'s, T> {
  #[inline]
  fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
    for t in iter {
      self.push(t)
    }
  }
}

impl<'s, T> IntoIterator for SliceVec<'s, T> {
  type Item = &'s mut T;
  type IntoIter = core::slice::IterMut<'s, T>;
  #[inline(always)]
  fn into_iter(self) -> Self::IntoIter {
    self.data.iter_mut()
  }
}

impl<'s, T> PartialEq for SliceVec<'s, T>
where
  T: PartialEq,
{
  #[inline]
  fn eq(&self, other: &Self) -> bool {
    self.as_slice().eq(other.as_slice())
  }
}
impl<'s, T> Eq for SliceVec<'s, T> where T: Eq {}

impl<'s, T> PartialOrd for SliceVec<'s, T>
where
  T: PartialOrd,
{
  #[inline]
  fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
    self.as_slice().partial_cmp(other.as_slice())
  }
}
impl<'s, T> Ord for SliceVec<'s, T>
where
  T: Ord,
{
  #[inline]
  fn cmp(&self, other: &Self) -> core::cmp::Ordering {
    self.as_slice().cmp(other.as_slice())
  }
}

impl<'s, T> PartialEq<&[T]> for SliceVec<'s, T>
where
  T: PartialEq,
{
  #[inline]
  fn eq(&self, other: &&[T]) -> bool {
    self.as_slice().eq(*other)
  }
}

impl<'s, T> Hash for SliceVec<'s, T>
where
  T: Hash,
{
  #[inline]
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.as_slice().hash(state)
  }
}

#[cfg(feature = "experimental_write_impl")]
impl<'s> core::fmt::Write for SliceVec<'s, u8> {
  fn write_str(&mut self, s: &str) -> core::fmt::Result {
    let my_len = self.len();
    let str_len = s.as_bytes().len();
    if my_len + str_len <= self.capacity() {
      let remainder = &mut self.data[my_len..];
      let target = &mut remainder[..str_len];
      target.copy_from_slice(s.as_bytes());
      Ok(())
    } else {
      Err(core::fmt::Error)
    }
  }
}

// // // // // // // //
// Formatting impls
// // // // // // // //

impl<'s, T> Binary for SliceVec<'s, T>
where
  T: Binary,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    if f.alternate() {
      write!(f, "\n    ")?;
    }
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ",{}", if f.alternate() { "\n    " } else { " " })?;
      }
      Binary::fmt(elem, f)?;
    }
    if f.alternate() {
      write!(f, ",\n")?;
    }
    write!(f, "]")
  }
}

impl<'s, T> Debug for SliceVec<'s, T>
where
  T: Debug,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    if f.alternate() && !self.is_empty() {
      write!(f, "\n    ")?;
    }
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ",{}", if f.alternate() { "\n    " } else { " " })?;
      }
      Debug::fmt(elem, f)?;
    }
    if f.alternate() && !self.is_empty() {
      write!(f, ",\n")?;
    }
    write!(f, "]")
  }
}

impl<'s, T> Display for SliceVec<'s, T>
where
  T: Display,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    if f.alternate() {
      write!(f, "\n    ")?;
    }
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ",{}", if f.alternate() { "\n    " } else { " " })?;
      }
      Display::fmt(elem, f)?;
    }
    if f.alternate() {
      write!(f, ",\n")?;
    }
    write!(f, "]")
  }
}

impl<'s, T> LowerExp for SliceVec<'s, T>
where
  T: LowerExp,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    if f.alternate() {
      write!(f, "\n    ")?;
    }
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ",{}", if f.alternate() { "\n    " } else { " " })?;
      }
      LowerExp::fmt(elem, f)?;
    }
    if f.alternate() {
      write!(f, ",\n")?;
    }
    write!(f, "]")
  }
}

impl<'s, T> LowerHex for SliceVec<'s, T>
where
  T: LowerHex,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    if f.alternate() {
      write!(f, "\n    ")?;
    }
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ",{}", if f.alternate() { "\n    " } else { " " })?;
      }
      LowerHex::fmt(elem, f)?;
    }
    if f.alternate() {
      write!(f, ",\n")?;
    }
    write!(f, "]")
  }
}

impl<'s, T> Octal for SliceVec<'s, T>
where
  T: Octal,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    if f.alternate() {
      write!(f, "\n    ")?;
    }
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ",{}", if f.alternate() { "\n    " } else { " " })?;
      }
      Octal::fmt(elem, f)?;
    }
    if f.alternate() {
      write!(f, ",\n")?;
    }
    write!(f, "]")
  }
}

impl<'s, T> Pointer for SliceVec<'s, T>
where
  T: Pointer,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    if f.alternate() {
      write!(f, "\n    ")?;
    }
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ",{}", if f.alternate() { "\n    " } else { " " })?;
      }
      Pointer::fmt(elem, f)?;
    }
    if f.alternate() {
      write!(f, ",\n")?;
    }
    write!(f, "]")
  }
}

impl<'s, T> UpperExp for SliceVec<'s, T>
where
  T: UpperExp,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    if f.alternate() {
      write!(f, "\n    ")?;
    }
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ",{}", if f.alternate() { "\n    " } else { " " })?;
      }
      UpperExp::fmt(elem, f)?;
    }
    if f.alternate() {
      write!(f, ",\n")?;
    }
    write!(f, "]")
  }
}

impl<'s, T> UpperHex for SliceVec<'s, T>
where
  T: UpperHex,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
    write!(f, "[")?;
    if f.alternate() {
      write!(f, "\n    ")?;
    }
    for (i, elem) in self.iter().enumerate() {
      if i > 0 {
        write!(f, ",{}", if f.alternate() { "\n    " } else { " " })?;
      }
      UpperHex::fmt(elem, f)?;
    }
    if f.alternate() {
      write!(f, ",\n")?;
    }
    write!(f, "]")
  }
}
