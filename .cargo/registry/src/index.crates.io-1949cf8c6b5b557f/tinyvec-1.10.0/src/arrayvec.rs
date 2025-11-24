use super::*;
use core::convert::{TryFrom, TryInto};

#[cfg(feature = "serde")]
use core::marker::PhantomData;
#[cfg(feature = "serde")]
use serde::de::{
  Deserialize, Deserializer, Error as DeserializeError, SeqAccess, Visitor,
};
#[cfg(feature = "serde")]
use serde::ser::{Serialize, SerializeSeq, Serializer};

/// Helper to make an `ArrayVec`.
///
/// You specify the backing array type, and optionally give all the elements you
/// want to initially place into the array.
///
/// ```rust
/// use tinyvec::*;
///
/// // The backing array type can be specified in the macro call
/// let empty_av = array_vec!([u8; 16]);
/// let some_ints = array_vec!([i32; 4] => 1, 2, 3);
///
/// // Or left to inference
/// let empty_av: ArrayVec<[u8; 10]> = array_vec!();
/// let some_ints: ArrayVec<[u8; 10]> = array_vec!(5, 6, 7, 8);
/// ```
#[macro_export]
macro_rules! array_vec {
  ($array_type:ty => $($elem:expr),* $(,)?) => {
    {
      let mut av: $crate::ArrayVec<$array_type> = Default::default();
      $( av.push($elem); )*
      av
    }
  };
  ($array_type:ty) => {
    $crate::ArrayVec::<$array_type>::default()
  };
  ($($elem:expr),*) => {
    $crate::array_vec!(_ => $($elem),*)
  };
  ($elem:expr; $n:expr) => {
    $crate::ArrayVec::from([$elem; $n])
  };
  () => {
    $crate::array_vec!(_)
  };
}

/// An array-backed, vector-like data structure.
///
/// * `ArrayVec` has a fixed capacity, equal to the minimum of the array size
///   and `u16::MAX`. Note that not all capacities are necessarily supported by
///   default. See comments in [`Array`].
/// * `ArrayVec` has a variable length, as you add and remove elements. Attempts
///   to fill the vec beyond its capacity will cause a panic.
/// * All of the vec's array slots are always initialized in terms of Rust's
///   memory model. When you remove a element from a location, the old value at
///   that location is replaced with the type's default value.
///
/// The overall API of this type is intended to, as much as possible, emulate
/// the API of the [`Vec`](https://doc.rust-lang.org/alloc/vec/struct.Vec.html)
/// type.
///
/// ## Construction
///
/// You can use the `array_vec!` macro similarly to how you might use the `vec!`
/// macro. Specify the array type, then optionally give all the initial values
/// you want to have.
/// ```rust
/// # use tinyvec::*;
/// let some_ints = array_vec!([i32; 4] => 1, 2, 3);
/// assert_eq!(some_ints.len(), 3);
/// ```
///
/// The [`default`](ArrayVec::new) for an `ArrayVec` is to have a default
/// array with length 0. The [`new`](ArrayVec::new) method is the same as
/// calling `default`
/// ```rust
/// # use tinyvec::*;
/// let some_ints = ArrayVec::<[i32; 7]>::default();
/// assert_eq!(some_ints.len(), 0);
///
/// let more_ints = ArrayVec::<[i32; 7]>::new();
/// assert_eq!(some_ints, more_ints);
/// ```
///
/// If you have an array and want the _whole thing_ so count as being "in" the
/// new `ArrayVec` you can use one of the `from` implementations. If you want
/// _part of_ the array then you can use
/// [`from_array_len`](ArrayVec::from_array_len):
/// ```rust
/// # use tinyvec::*;
/// let some_ints = ArrayVec::from([5, 6, 7, 8]);
/// assert_eq!(some_ints.len(), 4);
///
/// let more_ints = ArrayVec::from_array_len([5, 6, 7, 8], 2);
/// assert_eq!(more_ints.len(), 2);
///
/// let no_ints: ArrayVec<[u8; 5]> = ArrayVec::from_array_empty([1, 2, 3, 4, 5]);
/// assert_eq!(no_ints.len(), 0);
/// ```
#[repr(C)]
pub struct ArrayVec<A> {
  len: u16,
  pub(crate) data: A,
}

impl<A> Clone for ArrayVec<A>
where
  A: Array + Clone,
  A::Item: Clone,
{
  #[inline]
  fn clone(&self) -> Self {
    Self { data: self.data.clone(), len: self.len }
  }

  #[inline]
  fn clone_from(&mut self, o: &Self) {
    let iter = self
      .data
      .as_slice_mut()
      .iter_mut()
      .zip(o.data.as_slice())
      .take(self.len.max(o.len) as usize);
    for (dst, src) in iter {
      dst.clone_from(src)
    }
    if let Some(to_drop) =
      self.data.as_slice_mut().get_mut((o.len as usize)..(self.len as usize))
    {
      to_drop.iter_mut().for_each(|x| drop(core::mem::take(x)));
    }
    self.len = o.len;
  }
}

impl<A> Copy for ArrayVec<A>
where
  A: Array + Copy,
  A::Item: Copy,
{
}

impl<A: Array> Default for ArrayVec<A> {
  #[inline]
  fn default() -> Self {
    Self { len: 0, data: A::default() }
  }
}

impl<A: Array> Deref for ArrayVec<A> {
  type Target = [A::Item];
  #[inline(always)]
  fn deref(&self) -> &Self::Target {
    &self.data.as_slice()[..self.len as usize]
  }
}

impl<A: Array> DerefMut for ArrayVec<A> {
  #[inline(always)]
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.data.as_slice_mut()[..self.len as usize]
  }
}

impl<A: Array, I: SliceIndex<[A::Item]>> Index<I> for ArrayVec<A> {
  type Output = <I as SliceIndex<[A::Item]>>::Output;
  #[inline(always)]
  fn index(&self, index: I) -> &Self::Output {
    &self.deref()[index]
  }
}

impl<A: Array, I: SliceIndex<[A::Item]>> IndexMut<I> for ArrayVec<A> {
  #[inline(always)]
  fn index_mut(&mut self, index: I) -> &mut Self::Output {
    &mut self.deref_mut()[index]
  }
}

#[cfg(feature = "serde")]
#[cfg_attr(docs_rs, doc(cfg(feature = "serde")))]
impl<A: Array> Serialize for ArrayVec<A>
where
  A::Item: Serialize,
{
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut seq = serializer.serialize_seq(Some(self.len()))?;
    for element in self.iter() {
      seq.serialize_element(element)?;
    }
    seq.end()
  }
}

#[cfg(feature = "serde")]
#[cfg_attr(docs_rs, doc(cfg(feature = "serde")))]
impl<'de, A: Array> Deserialize<'de> for ArrayVec<A>
where
  A::Item: Deserialize<'de>,
{
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_seq(ArrayVecVisitor(PhantomData))
  }
}

#[cfg(feature = "borsh")]
#[cfg_attr(docs_rs, doc(cfg(feature = "borsh")))]
impl<A: Array> borsh::BorshSerialize for ArrayVec<A>
where
  <A as Array>::Item: borsh::BorshSerialize,
{
  fn serialize<W: borsh::io::Write>(
    &self, writer: &mut W,
  ) -> borsh::io::Result<()> {
    <usize as borsh::BorshSerialize>::serialize(&self.len(), writer)?;
    for elem in self.iter() {
      <<A as Array>::Item as borsh::BorshSerialize>::serialize(elem, writer)?;
    }
    Ok(())
  }
}

#[cfg(feature = "borsh")]
#[cfg_attr(docs_rs, doc(cfg(feature = "borsh")))]
impl<A: Array> borsh::BorshDeserialize for ArrayVec<A>
where
  <A as Array>::Item: borsh::BorshDeserialize,
{
  fn deserialize_reader<R: borsh::io::Read>(
    reader: &mut R,
  ) -> borsh::io::Result<Self> {
    let len = <usize as borsh::BorshDeserialize>::deserialize_reader(reader)?;
    let mut new_arrayvec = Self::default();

    for idx in 0..len {
      let value =
        <<A as Array>::Item as borsh::BorshDeserialize>::deserialize_reader(
          reader,
        )?;
      if idx >= new_arrayvec.capacity() {
        return Err(borsh::io::Error::new(
          borsh::io::ErrorKind::InvalidData,
          "invalid ArrayVec length",
        ));
      }
      new_arrayvec.push(value)
    }

    Ok(new_arrayvec)
  }
}

#[cfg(feature = "arbitrary")]
#[cfg_attr(docs_rs, doc(cfg(feature = "arbitrary")))]
impl<'a, A> arbitrary::Arbitrary<'a> for ArrayVec<A>
where
  A: Array,
  A::Item: arbitrary::Arbitrary<'a>,
{
  fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
    let max_len = A::CAPACITY.min(u16::MAX as usize) as u16;
    let len = u.int_in_range::<u16>(0..=max_len)?;
    let mut self_: Self = Default::default();
    for _ in 0..len {
      self_.push(u.arbitrary()?);
    }
    Ok(self_)
  }

  fn size_hint(depth: usize) -> (usize, Option<usize>) {
    arbitrary::size_hint::recursion_guard(depth, |depth| {
      let max_len = A::CAPACITY.min(u16::MAX as usize);
      let inner = A::Item::size_hint(depth).1;
      (0, inner.map(|inner| 2 + max_len * inner))
    })
  }
}

impl<A: Array> ArrayVec<A> {
  /// Move all values from `other` into this vec.
  ///
  /// ## Panics
  /// * If the vec overflows its capacity
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut av = array_vec!([i32; 10] => 1, 2, 3);
  /// let mut av2 = array_vec!([i32; 10] => 4, 5, 6);
  /// av.append(&mut av2);
  /// assert_eq!(av, &[1, 2, 3, 4, 5, 6][..]);
  /// assert_eq!(av2, &[][..]);
  /// ```
  #[inline]
  pub fn append(&mut self, other: &mut Self) {
    assert!(
      self.try_append(other).is_none(),
      "ArrayVec::append> total length {} exceeds capacity {}!",
      self.len() + other.len(),
      A::CAPACITY
    );
  }

  /// Move all values from `other` into this vec.
  /// If appending would overflow the capacity, Some(other) is returned.
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut av = array_vec!([i32; 7] => 1, 2, 3);
  /// let mut av2 = array_vec!([i32; 7] => 4, 5, 6);
  /// av.append(&mut av2);
  /// assert_eq!(av, &[1, 2, 3, 4, 5, 6][..]);
  /// assert_eq!(av2, &[][..]);
  ///
  /// let mut av3 = array_vec!([i32; 7] => 7, 8, 9);
  /// assert!(av.try_append(&mut av3).is_some());
  /// assert_eq!(av, &[1, 2, 3, 4, 5, 6][..]);
  /// assert_eq!(av3, &[7, 8, 9][..]);
  /// ```
  #[inline]
  pub fn try_append<'other>(
    &mut self, other: &'other mut Self,
  ) -> Option<&'other mut Self> {
    let new_len = self.len() + other.len();
    if new_len > A::CAPACITY {
      return Some(other);
    }

    let iter = other.iter_mut().map(core::mem::take);
    for item in iter {
      self.push(item);
    }

    other.set_len(0);

    return None;
  }

  /// A `*mut` pointer to the backing array.
  ///
  /// ## Safety
  ///
  /// This pointer has provenance over the _entire_ backing array.
  #[inline(always)]
  #[must_use]
  pub fn as_mut_ptr(&mut self) -> *mut A::Item {
    self.data.as_slice_mut().as_mut_ptr()
  }

  /// Performs a `deref_mut`, into unique slice form.
  #[inline(always)]
  #[must_use]
  pub fn as_mut_slice(&mut self) -> &mut [A::Item] {
    self.deref_mut()
  }

  /// A `*const` pointer to the backing array.
  ///
  /// ## Safety
  ///
  /// This pointer has provenance over the _entire_ backing array.
  #[inline(always)]
  #[must_use]
  pub fn as_ptr(&self) -> *const A::Item {
    self.data.as_slice().as_ptr()
  }

  /// Performs a `deref`, into shared slice form.
  #[inline(always)]
  #[must_use]
  pub fn as_slice(&self) -> &[A::Item] {
    self.deref()
  }

  /// The capacity of the `ArrayVec`.
  ///
  /// This is fixed based on the array type, but can't yet be made a `const fn`
  /// on Stable Rust.
  #[inline(always)]
  #[must_use]
  pub fn capacity(&self) -> usize {
    // Note: This shouldn't use A::CAPACITY, because unsafe code can't rely on
    // any Array invariants. This ensures that at the very least, the returned
    // value is a valid length for a subslice of the backing array.
    self.data.as_slice().len().min(u16::MAX as usize)
  }

  /// Truncates the `ArrayVec` down to length 0.
  #[inline(always)]
  pub fn clear(&mut self) {
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
  /// let mut av = array_vec!([i32; 4] => 1, 2, 3);
  /// let av2: ArrayVec<[i32; 4]> = av.drain(1..).collect();
  /// assert_eq!(av.as_slice(), &[1][..]);
  /// assert_eq!(av2.as_slice(), &[2, 3][..]);
  ///
  /// av.drain(..);
  /// assert_eq!(av.as_slice(), &[]);
  /// ```
  #[inline]
  pub fn drain<R>(&mut self, range: R) -> ArrayVecDrain<'_, A::Item>
  where
    R: RangeBounds<usize>,
  {
    ArrayVecDrain::new(self, range)
  }

  /// Returns the inner array of the `ArrayVec`.
  ///
  /// This returns the full array, even if the `ArrayVec` length is currently
  /// less than that.
  ///
  /// ## Example
  ///
  /// ```rust
  /// # use tinyvec::{array_vec, ArrayVec};
  /// let mut favorite_numbers = array_vec!([i32; 5] => 87, 48, 33, 9, 26);
  /// assert_eq!(favorite_numbers.clone().into_inner(), [87, 48, 33, 9, 26]);
  ///
  /// favorite_numbers.pop();
  /// assert_eq!(favorite_numbers.into_inner(), [87, 48, 33, 9, 0]);
  /// ```
  ///
  /// A use for this function is to build an array from an iterator by first
  /// collecting it into an `ArrayVec`.
  ///
  /// ```rust
  /// # use tinyvec::ArrayVec;
  /// let arr_vec: ArrayVec<[i32; 10]> = (1..=3).cycle().take(10).collect();
  /// let inner = arr_vec.into_inner();
  /// assert_eq!(inner, [1, 2, 3, 1, 2, 3, 1, 2, 3, 1]);
  /// ```
  #[inline]
  pub fn into_inner(self) -> A {
    self.data
  }

  /// Clone each element of the slice into this `ArrayVec`.
  ///
  /// ## Panics
  /// * If the `ArrayVec` would overflow, this will panic.
  #[inline]
  pub fn extend_from_slice(&mut self, sli: &[A::Item])
  where
    A::Item: Clone,
  {
    if sli.is_empty() {
      return;
    }

    let new_len = self.len as usize + sli.len();
    assert!(
      new_len <= A::CAPACITY,
      "ArrayVec::extend_from_slice> total length {} exceeds capacity {}!",
      new_len,
      A::CAPACITY
    );

    let target = &mut self.data.as_slice_mut()[self.len as usize..new_len];
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
  /// let mut av = array_vec!([i32; 4]);
  /// let mut to_inf = av.fill(0..);
  /// assert_eq!(&av[..], [0, 1, 2, 3]);
  /// assert_eq!(to_inf.next(), Some(4));
  /// ```
  #[inline]
  pub fn fill<I: IntoIterator<Item = A::Item>>(
    &mut self, iter: I,
  ) -> I::IntoIter {
    // If this is written as a call to push for each element in iter, the
    // compiler emits code that updates the length for every element. The
    // additional complexity from that length update is worth nearly 2x in
    // the runtime of this function.
    let mut iter = iter.into_iter();
    let mut pushed = 0;
    let to_take = self.capacity() - self.len();
    let target = &mut self.data.as_slice_mut()[self.len as usize..];
    for element in iter.by_ref().take(to_take) {
      target[pushed] = element;
      pushed += 1;
    }
    self.len += pushed as u16;
    iter
  }

  /// Wraps up an array and uses the given length as the initial length.
  ///
  /// If you want to simply use the full array, use `from` instead.
  ///
  /// ## Panics
  ///
  /// * The length specified must be less than or equal to the capacity of the
  ///   array.
  #[inline]
  #[must_use]
  #[allow(clippy::match_wild_err_arm)]
  pub fn from_array_len(data: A, len: usize) -> Self {
    match Self::try_from_array_len(data, len) {
      Ok(out) => out,
      Err(_) => panic!(
        "ArrayVec::from_array_len> length {} exceeds capacity {}!",
        len,
        A::CAPACITY
      ),
    }
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
  /// use tinyvec::*;
  /// let mut av = array_vec!([i32; 10] => 1, 2, 3);
  /// av.insert(1, 4);
  /// assert_eq!(av.as_slice(), &[1, 4, 2, 3]);
  /// av.insert(4, 5);
  /// assert_eq!(av.as_slice(), &[1, 4, 2, 3, 5]);
  /// ```
  #[inline]
  pub fn insert(&mut self, index: usize, item: A::Item) {
    let x = self.try_insert(index, item);
    assert!(x.is_none(), "ArrayVec::insert> capacity overflow!");
  }

  /// Tries to insert an item at the position given, moving all following
  /// elements +1 index.
  /// Returns back the element if the capacity is exhausted,
  /// otherwise returns None.
  ///
  /// ## Panics
  /// * If `index` > `len`
  ///
  /// ## Example
  /// ```rust
  /// use tinyvec::*;
  /// let mut av = array_vec!([&'static str; 4] => "one", "two", "three");
  /// av.insert(1, "four");
  /// assert_eq!(av.as_slice(), &["one", "four", "two", "three"]);
  /// assert_eq!(av.try_insert(4, "five"), Some("five"));
  /// ```
  #[inline]
  pub fn try_insert(
    &mut self, index: usize, mut item: A::Item,
  ) -> Option<A::Item> {
    assert!(
      index <= self.len as usize,
      "ArrayVec::try_insert> index {} is out of bounds {}",
      index,
      self.len
    );

    // A previous implementation used self.try_push and slice::rotate_right
    // rotate_right and rotate_left generate a huge amount of code and fail to
    // inline; calling them here incurs the cost of all the cases they
    // handle even though we're rotating a usually-small array by a constant
    // 1 offset. This swap-based implementation benchmarks much better for
    // small array lengths in particular.

    if (self.len as usize) < A::CAPACITY {
      self.len += 1;
    } else {
      return Some(item);
    }

    let target = &mut self.as_mut_slice()[index..];
    #[allow(clippy::needless_range_loop)]
    for i in 0..target.len() {
      core::mem::swap(&mut item, &mut target[i]);
    }
    return None;
  }

  /// Checks if the length is 0.
  #[inline(always)]
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len == 0
  }

  /// The length of the `ArrayVec` (in elements).
  #[inline(always)]
  #[must_use]
  pub fn len(&self) -> usize {
    self.len as usize
  }

  /// Makes a new, empty `ArrayVec`.
  #[inline(always)]
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Remove and return the last element of the vec, if there is one.
  ///
  /// ## Failure
  /// * If the vec is empty you get `None`.
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut av = array_vec!([i32; 10] => 1, 2);
  /// assert_eq!(av.pop(), Some(2));
  /// assert_eq!(av.pop(), Some(1));
  /// assert_eq!(av.pop(), None);
  /// ```
  #[inline]
  pub fn pop(&mut self) -> Option<A::Item> {
    if self.len > 0 {
      self.len -= 1;
      let out =
        core::mem::take(&mut self.data.as_slice_mut()[self.len as usize]);
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
  /// let mut av = array_vec!([i32; 2]);
  /// assert_eq!(&av[..], []);
  /// av.push(1);
  /// assert_eq!(&av[..], [1]);
  /// av.push(2);
  /// assert_eq!(&av[..], [1, 2]);
  /// // av.push(3); this would overflow the ArrayVec and panic!
  /// ```
  #[inline(always)]
  pub fn push(&mut self, val: A::Item) {
    let x = self.try_push(val);
    assert!(x.is_none(), "ArrayVec::push> capacity overflow!");
  }

  /// Tries to place an element onto the end of the vec.\
  /// Returns back the element if the capacity is exhausted,
  /// otherwise returns None.
  /// ```rust
  /// # use tinyvec::*;
  /// let mut av = array_vec!([i32; 2]);
  /// assert_eq!(av.as_slice(), []);
  /// assert_eq!(av.try_push(1), None);
  /// assert_eq!(&av[..], [1]);
  /// assert_eq!(av.try_push(2), None);
  /// assert_eq!(&av[..], [1, 2]);
  /// assert_eq!(av.try_push(3), Some(3));
  /// ```
  #[inline(always)]
  pub fn try_push(&mut self, val: A::Item) -> Option<A::Item> {
    debug_assert!(self.len as usize <= A::CAPACITY);

    let itemref = match self.data.as_slice_mut().get_mut(self.len as usize) {
      None => return Some(val),
      Some(x) => x,
    };

    *itemref = val;
    self.len += 1;
    return None;
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
  /// let mut av = array_vec!([i32; 4] => 1, 2, 3);
  /// assert_eq!(av.remove(1), 2);
  /// assert_eq!(&av[..], [1, 3]);
  /// ```
  #[inline]
  pub fn remove(&mut self, index: usize) -> A::Item {
    let targets: &mut [A::Item] = &mut self.deref_mut()[index..];
    let item = core::mem::take(&mut targets[0]);

    // A previous implementation used rotate_left
    // rotate_right and rotate_left generate a huge amount of code and fail to
    // inline; calling them here incurs the cost of all the cases they
    // handle even though we're rotating a usually-small array by a constant
    // 1 offset. This swap-based implementation benchmarks much better for
    // small array lengths in particular.

    for i in 0..targets.len() - 1 {
      targets.swap(i, i + 1);
    }
    self.len -= 1;
    item
  }

  /// As [`resize_with`](ArrayVec::resize_with)
  /// and it clones the value as the closure.
  ///
  /// ## Example
  ///
  /// ```rust
  /// # use tinyvec::*;
  ///
  /// let mut av = array_vec!([&str; 10] => "hello");
  /// av.resize(3, "world");
  /// assert_eq!(&av[..], ["hello", "world", "world"]);
  ///
  /// let mut av = array_vec!([i32; 10] => 1, 2, 3, 4);
  /// av.resize(2, 0);
  /// assert_eq!(&av[..], [1, 2]);
  /// ```
  #[inline]
  pub fn resize(&mut self, new_len: usize, new_val: A::Item)
  where
    A::Item: Clone,
  {
    self.resize_with(new_len, || new_val.clone())
  }

  /// Resize the vec to the new length.
  ///
  /// If it needs to be longer, it's filled with repeated calls to the provided
  /// function. If it needs to be shorter, it's truncated.
  ///
  /// ## Example
  ///
  /// ```rust
  /// # use tinyvec::*;
  ///
  /// let mut av = array_vec!([i32; 10] => 1, 2, 3);
  /// av.resize_with(5, Default::default);
  /// assert_eq!(&av[..], [1, 2, 3, 0, 0]);
  ///
  /// let mut av = array_vec!([i32; 10]);
  /// let mut p = 1;
  /// av.resize_with(4, || {
  ///   p *= 2;
  ///   p
  /// });
  /// assert_eq!(&av[..], [2, 4, 8, 16]);
  /// ```
  #[inline]
  pub fn resize_with<F: FnMut() -> A::Item>(
    &mut self, new_len: usize, mut f: F,
  ) {
    match new_len.checked_sub(self.len as usize) {
      None => self.truncate(new_len),
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
  /// let mut av = array_vec!([i32; 10] => 1, 1, 2, 3, 3, 4);
  /// av.retain(|&x| x % 2 == 0);
  /// assert_eq!(&av[..], [2, 4]);
  /// ```
  #[inline]
  pub fn retain<F: FnMut(&A::Item) -> bool>(&mut self, mut acceptable: F) {
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

    let mut rest = JoinOnDrop {
      items: &mut self.data.as_slice_mut()[..self.len as usize],
      done_end: 0,
      tail_start: 0,
    };

    let len = self.len as usize;
    for idx in 0..len {
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

  /// Retains only the elements specified by the predicate, passing a mutable
  /// reference to it.
  ///
  /// In other words, remove all elements e such that f(&mut e) returns false.
  /// This method operates in place, visiting each element exactly once in the
  /// original order, and preserves the order of the retained elements.
  ///
  ///
  /// ## Example
  ///
  /// ```rust
  /// # use tinyvec::*;
  ///
  /// let mut av = array_vec!([i32; 10] => 1, 1, 2, 3, 3, 4);
  /// av.retain_mut(|x| if *x % 2 == 0 { *x *= 2; true } else { false });
  /// assert_eq!(&av[..], [4, 8]);
  /// ```
  #[inline]
  pub fn retain_mut<F>(&mut self, mut acceptable: F)
  where
    F: FnMut(&mut A::Item) -> bool,
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

    let mut rest = JoinOnDrop {
      items: &mut self.data.as_slice_mut()[..self.len as usize],
      done_end: 0,
      tail_start: 0,
    };

    let len = self.len as usize;
    for idx in 0..len {
      // Loop start invariant: idx = rest.done_end + rest.tail_start
      if !acceptable(&mut rest.items[idx]) {
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
    if new_len > A::CAPACITY {
      // Note(Lokathor): Technically we don't have to panic here, and we could
      // just let some other call later on trigger a panic on accident when the
      // length is wrong. However, it's a lot easier to catch bugs when things
      // are more "fail-fast".
      panic!(
        "ArrayVec::set_len> new length {} exceeds capacity {}",
        new_len,
        A::CAPACITY
      )
    }

    let new_len: u16 = new_len
      .try_into()
      .expect("ArrayVec::set_len> new length is not in range 0..=u16::MAX");
    self.len = new_len;
  }

  /// Splits the collection at the point given.
  ///
  /// * `[0, at)` stays in this vec
  /// * `[at, len)` ends up in the new vec.
  ///
  /// ## Panics
  /// * if at > len
  ///
  /// ## Example
  ///
  /// ```rust
  /// # use tinyvec::*;
  /// let mut av = array_vec!([i32; 4] => 1, 2, 3);
  /// let av2 = av.split_off(1);
  /// assert_eq!(&av[..], [1]);
  /// assert_eq!(&av2[..], [2, 3]);
  /// ```
  #[inline]
  pub fn split_off(&mut self, at: usize) -> Self {
    // FIXME: should this just use drain into the output?
    if at > self.len() {
      panic!(
        "ArrayVec::split_off> at value {} exceeds length of {}",
        at, self.len
      );
    }
    let mut new = Self::default();
    let moves = &mut self.as_mut_slice()[at..];
    let split_len = moves.len();
    let targets = &mut new.data.as_slice_mut()[..split_len];
    moves.swap_with_slice(targets);

    /* moves.len() <= u16::MAX, so these are surely in u16 range */
    new.len = split_len as u16;
    self.len = at as u16;
    new
  }

  /// Creates a splicing iterator that removes the specified range in the
  /// vector, yields the removed items, and replaces them with elements from
  /// the provided iterator.
  ///
  /// `splice` fuses the provided iterator, so elements after the first `None`
  /// are ignored.
  ///
  /// ## Panics
  /// * If the start is greater than the end.
  /// * If the end is past the edge of the vec.
  /// * If the provided iterator panics.
  /// * If the new length would overflow the capacity of the array. Because
  ///   `ArrayVecSplice` adds elements to this vec in its destructor when
  ///   necessary, this panic would occur when it is dropped.
  ///
  /// ## Example
  /// ```rust
  /// use tinyvec::*;
  /// let mut av = array_vec!([i32; 4] => 1, 2, 3);
  /// let av2: ArrayVec<[i32; 4]> = av.splice(1.., 4..=6).collect();
  /// assert_eq!(av.as_slice(), &[1, 4, 5, 6][..]);
  /// assert_eq!(av2.as_slice(), &[2, 3][..]);
  ///
  /// av.splice(.., None);
  /// assert_eq!(av.as_slice(), &[]);
  /// ```
  #[inline]
  pub fn splice<R, I>(
    &mut self, range: R, replacement: I,
  ) -> ArrayVecSplice<'_, A, core::iter::Fuse<I::IntoIter>>
  where
    R: RangeBounds<usize>,
    I: IntoIterator<Item = A::Item>,
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
      Bound::Unbounded => self.len(),
    };
    assert!(
      start <= end,
      "ArrayVec::splice> Illegal range, {} to {}",
      start,
      end
    );
    assert!(
      end <= self.len(),
      "ArrayVec::splice> Range ends at {} but length is only {}!",
      end,
      self.len()
    );

    ArrayVecSplice {
      removal_start: start,
      removal_end: end,
      parent: self,
      replacement: replacement.into_iter().fuse(),
    }
  }

  /// Remove an element, swapping the end of the vec into its place.
  ///
  /// ## Panics
  /// * If the index is out of bounds.
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut av = array_vec!([&str; 4] => "foo", "bar", "quack", "zap");
  ///
  /// assert_eq!(av.swap_remove(1), "bar");
  /// assert_eq!(&av[..], ["foo", "zap", "quack"]);
  ///
  /// assert_eq!(av.swap_remove(0), "foo");
  /// assert_eq!(&av[..], ["quack", "zap"]);
  /// ```
  #[inline]
  pub fn swap_remove(&mut self, index: usize) -> A::Item {
    assert!(
      index < self.len(),
      "ArrayVec::swap_remove> index {} is out of bounds {}",
      index,
      self.len
    );
    if index == self.len() - 1 {
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
  pub fn truncate(&mut self, new_len: usize) {
    if new_len >= self.len as usize {
      return;
    }

    if needs_drop::<A::Item>() {
      let len = self.len as usize;
      self.data.as_slice_mut()[new_len..len]
        .iter_mut()
        .map(core::mem::take)
        .for_each(drop);
    }

    /* new_len is less than self.len */
    self.len = new_len as u16;
  }

  /// Wraps an array, using the given length as the starting length.
  ///
  /// If you want to use the whole length of the array, you can just use the
  /// `From` impl.
  ///
  /// ## Failure
  ///
  /// If the given length is greater than the capacity of the array this will
  /// error, and you'll get the array back in the `Err`.
  #[inline]
  #[cfg(not(feature = "latest_stable_rust"))]
  pub fn try_from_array_len(data: A, len: usize) -> Result<Self, A> {
    /* Note(Soveu): Should we allow A::CAPACITY > u16::MAX for now? */
    if len <= A::CAPACITY {
      Ok(Self { data, len: len as u16 })
    } else {
      Err(data)
    }
  }

  /// Wraps an array, using the given length as the starting length.
  ///
  /// If you want to use the whole length of the array, you can just use the
  /// `From` impl.
  ///
  /// ## Failure
  ///
  /// If the given length is greater than the capacity of the array this will
  /// error, and you'll get the array back in the `Err`.
  #[inline]
  #[cfg(feature = "latest_stable_rust")]
  pub const fn try_from_array_len(data: A, len: usize) -> Result<Self, A> {
    /* Note(Soveu): Should we allow A::CAPACITY > u16::MAX for now? */
    if len <= A::CAPACITY {
      Ok(Self { data, len: len as u16 })
    } else {
      Err(data)
    }
  }
}

impl<A> ArrayVec<A> {
  /// Wraps up an array as a new empty `ArrayVec`.
  ///
  /// If you want to simply use the full array, use `from` instead.
  ///
  /// ## Examples
  ///
  /// This method in particular allows to create values for statics:
  ///
  /// ```rust
  /// # use tinyvec::ArrayVec;
  /// static DATA: ArrayVec<[u8; 5]> = ArrayVec::from_array_empty([0; 5]);
  /// assert_eq!(DATA.len(), 0);
  /// ```
  ///
  /// But of course it is just an normal empty `ArrayVec`:
  ///
  /// ```rust
  /// # use tinyvec::ArrayVec;
  /// let mut data = ArrayVec::from_array_empty([1, 2, 3, 4]);
  /// assert_eq!(&data[..], &[]);
  /// data.push(42);
  /// assert_eq!(&data[..], &[42]);
  /// ```
  #[inline]
  #[must_use]
  pub const fn from_array_empty(data: A) -> Self {
    Self { data, len: 0 }
  }
}

#[cfg(feature = "grab_spare_slice")]
impl<A: Array> ArrayVec<A> {
  /// Obtain the shared slice of the array _after_ the active memory.
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut av = array_vec!([i32; 4]);
  /// assert_eq!(av.grab_spare_slice().len(), 4);
  /// av.push(10);
  /// av.push(11);
  /// av.push(12);
  /// av.push(13);
  /// assert_eq!(av.grab_spare_slice().len(), 0);
  /// ```
  #[inline(always)]
  pub fn grab_spare_slice(&self) -> &[A::Item] {
    &self.data.as_slice()[self.len as usize..]
  }

  /// Obtain the mutable slice of the array _after_ the active memory.
  ///
  /// ## Example
  /// ```rust
  /// # use tinyvec::*;
  /// let mut av = array_vec!([i32; 4]);
  /// assert_eq!(av.grab_spare_slice_mut().len(), 4);
  /// av.push(10);
  /// av.push(11);
  /// assert_eq!(av.grab_spare_slice_mut().len(), 2);
  /// ```
  #[inline(always)]
  pub fn grab_spare_slice_mut(&mut self) -> &mut [A::Item] {
    &mut self.data.as_slice_mut()[self.len as usize..]
  }
}

#[cfg(feature = "nightly_slice_partition_dedup")]
impl<A: Array> ArrayVec<A> {
  /// De-duplicates the vec contents.
  #[inline(always)]
  pub fn dedup(&mut self)
  where
    A::Item: PartialEq,
  {
    self.dedup_by(|a, b| a == b)
  }

  /// De-duplicates the vec according to the predicate given.
  #[inline(always)]
  pub fn dedup_by<F>(&mut self, same_bucket: F)
  where
    F: FnMut(&mut A::Item, &mut A::Item) -> bool,
  {
    let len = {
      let (dedup, _) = self.as_mut_slice().partition_dedup_by(same_bucket);
      dedup.len()
    };
    self.truncate(len);
  }

  /// De-duplicates the vec according to the key selector given.
  #[inline(always)]
  pub fn dedup_by_key<F, K>(&mut self, mut key: F)
  where
    F: FnMut(&mut A::Item) -> K,
    K: PartialEq,
  {
    self.dedup_by(|a, b| key(a) == key(b))
  }
}

impl<A> ArrayVec<A> {
  /// Returns the reference to the inner array of the `ArrayVec`.
  ///
  /// This returns the full array, even if the `ArrayVec` length is currently
  /// less than that.
  #[inline(always)]
  #[must_use]
  pub const fn as_inner(&self) -> &A {
    &self.data
  }
}

/// Splicing iterator for `ArrayVec`
/// See [`ArrayVec::splice`](ArrayVec::<A>::splice)
pub struct ArrayVecSplice<'p, A: Array, I: Iterator<Item = A::Item>> {
  parent: &'p mut ArrayVec<A>,
  removal_start: usize,
  removal_end: usize,
  replacement: I,
}

impl<'p, A: Array, I: Iterator<Item = A::Item>> Iterator
  for ArrayVecSplice<'p, A, I>
{
  type Item = A::Item;

  #[inline]
  fn next(&mut self) -> Option<A::Item> {
    if self.removal_start < self.removal_end {
      match self.replacement.next() {
        Some(replacement) => {
          let removed = core::mem::replace(
            &mut self.parent[self.removal_start],
            replacement,
          );
          self.removal_start += 1;
          Some(removed)
        }
        None => {
          let removed = self.parent.remove(self.removal_start);
          self.removal_end -= 1;
          Some(removed)
        }
      }
    } else {
      None
    }
  }

  #[inline]
  fn size_hint(&self) -> (usize, Option<usize>) {
    let len = self.len();
    (len, Some(len))
  }
}

impl<'p, A, I> ExactSizeIterator for ArrayVecSplice<'p, A, I>
where
  A: Array,
  I: Iterator<Item = A::Item>,
{
  #[inline]
  fn len(&self) -> usize {
    self.removal_end - self.removal_start
  }
}

impl<'p, A, I> FusedIterator for ArrayVecSplice<'p, A, I>
where
  A: Array,
  I: Iterator<Item = A::Item>,
{
}

impl<'p, A, I> DoubleEndedIterator for ArrayVecSplice<'p, A, I>
where
  A: Array,
  I: Iterator<Item = A::Item> + DoubleEndedIterator,
{
  #[inline]
  fn next_back(&mut self) -> Option<A::Item> {
    if self.removal_start < self.removal_end {
      match self.replacement.next_back() {
        Some(replacement) => {
          let removed = core::mem::replace(
            &mut self.parent[self.removal_end - 1],
            replacement,
          );
          self.removal_end -= 1;
          Some(removed)
        }
        None => {
          let removed = self.parent.remove(self.removal_end - 1);
          self.removal_end -= 1;
          Some(removed)
        }
      }
    } else {
      None
    }
  }
}

impl<'p, A: Array, I: Iterator<Item = A::Item>> Drop
  for ArrayVecSplice<'p, A, I>
{
  #[inline]
  fn drop(&mut self) {
    for _ in self.by_ref() {}

    // FIXME: reserve lower bound of size_hint

    for replacement in self.replacement.by_ref() {
      self.parent.insert(self.removal_end, replacement);
      self.removal_end += 1;
    }
  }
}

impl<A: Array> AsMut<[A::Item]> for ArrayVec<A> {
  #[inline(always)]
  fn as_mut(&mut self) -> &mut [A::Item] {
    &mut *self
  }
}

impl<A: Array> AsRef<[A::Item]> for ArrayVec<A> {
  #[inline(always)]
  fn as_ref(&self) -> &[A::Item] {
    &*self
  }
}

impl<A: Array> Borrow<[A::Item]> for ArrayVec<A> {
  #[inline(always)]
  fn borrow(&self) -> &[A::Item] {
    &*self
  }
}

impl<A: Array> BorrowMut<[A::Item]> for ArrayVec<A> {
  #[inline(always)]
  fn borrow_mut(&mut self) -> &mut [A::Item] {
    &mut *self
  }
}

impl<A: Array> Extend<A::Item> for ArrayVec<A> {
  #[inline]
  fn extend<T: IntoIterator<Item = A::Item>>(&mut self, iter: T) {
    for t in iter {
      self.push(t)
    }
  }
}

impl<A: Array> From<A> for ArrayVec<A> {
  #[inline(always)]
  /// The output has a length equal to the full array.
  ///
  /// If you want to select a length, use
  /// [`from_array_len`](ArrayVec::from_array_len)
  fn from(data: A) -> Self {
    let len: u16 = data
      .as_slice()
      .len()
      .try_into()
      .expect("ArrayVec::from> length must be in range 0..=u16::MAX");
    Self { len, data }
  }
}

/// The error type returned when a conversion from a slice to an [`ArrayVec`]
/// fails.
#[derive(Debug, Copy, Clone)]
pub struct TryFromSliceError(());

impl core::fmt::Display for TryFromSliceError {
  #[inline]
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    f.write_str("could not convert slice to ArrayVec")
  }
}

#[cfg(feature = "std")]
impl std::error::Error for TryFromSliceError {}

impl<T, A> TryFrom<&'_ [T]> for ArrayVec<A>
where
  T: Clone + Default,
  A: Array<Item = T>,
{
  type Error = TryFromSliceError;

  #[inline]
  /// The output has a length equal to that of the slice, with the same capacity
  /// as `A`.
  fn try_from(slice: &[T]) -> Result<Self, Self::Error> {
    if slice.len() > A::CAPACITY {
      Err(TryFromSliceError(()))
    } else {
      let mut arr = ArrayVec::new();
      // We do not use ArrayVec::extend_from_slice, because it looks like LLVM
      // fails to deduplicate all the length-checking logic between the
      // above if and the contents of that method, thus producing much
      // slower code. Unlike many of the other optimizations in this
      // crate, this one is worth keeping an eye on. I see no reason, for
      // any element type, that these should produce different code. But
      // they do. (rustc 1.51.0)
      arr.set_len(slice.len());
      arr.as_mut_slice().clone_from_slice(slice);
      Ok(arr)
    }
  }
}

impl<A: Array> FromIterator<A::Item> for ArrayVec<A> {
  #[inline]
  fn from_iter<T: IntoIterator<Item = A::Item>>(iter: T) -> Self {
    let mut av = Self::default();
    for i in iter {
      av.push(i)
    }
    av
  }
}

/// Iterator for consuming an `ArrayVec` and returning owned elements.
pub struct ArrayVecIterator<A: Array> {
  base: u16,
  tail: u16,
  data: A,
}

impl<A: Array> ArrayVecIterator<A> {
  /// Returns the remaining items of this iterator as a slice.
  #[inline]
  #[must_use]
  pub fn as_slice(&self) -> &[A::Item] {
    &self.data.as_slice()[self.base as usize..self.tail as usize]
  }
}
impl<A: Array> FusedIterator for ArrayVecIterator<A> {}
impl<A: Array> Iterator for ArrayVecIterator<A> {
  type Item = A::Item;
  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    let slice =
      &mut self.data.as_slice_mut()[self.base as usize..self.tail as usize];
    let itemref = slice.first_mut()?;
    self.base += 1;
    return Some(core::mem::take(itemref));
  }
  #[inline(always)]
  fn size_hint(&self) -> (usize, Option<usize>) {
    let s = self.tail - self.base;
    let s = s as usize;
    (s, Some(s))
  }
  #[inline(always)]
  fn count(self) -> usize {
    self.size_hint().0
  }
  #[inline]
  fn last(mut self) -> Option<Self::Item> {
    self.next_back()
  }
  #[inline]
  fn nth(&mut self, n: usize) -> Option<A::Item> {
    let slice = &mut self.data.as_slice_mut();
    let slice = &mut slice[self.base as usize..self.tail as usize];

    if let Some(x) = slice.get_mut(n) {
      /* n is in range [0 .. self.tail - self.base) so in u16 range */
      self.base += n as u16 + 1;
      return Some(core::mem::take(x));
    }

    self.base = self.tail;
    return None;
  }
}

impl<A: Array> DoubleEndedIterator for ArrayVecIterator<A> {
  #[inline]
  fn next_back(&mut self) -> Option<Self::Item> {
    let slice =
      &mut self.data.as_slice_mut()[self.base as usize..self.tail as usize];
    let item = slice.last_mut()?;
    self.tail -= 1;
    return Some(core::mem::take(item));
  }

  #[inline]
  fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
    let base = self.base as usize;
    let tail = self.tail as usize;
    let slice = &mut self.data.as_slice_mut()[base..tail];
    let n = n.saturating_add(1);

    if let Some(n) = slice.len().checked_sub(n) {
      let item = &mut slice[n];
      /* n is in [0..self.tail - self.base] range, so in u16 range */
      self.tail = self.base + n as u16;
      return Some(core::mem::take(item));
    }

    self.tail = self.base;
    return None;
  }
}

impl<A: Array> ExactSizeIterator for ArrayVecIterator<A> {
  #[inline]
  fn len(&self) -> usize {
    self.size_hint().0
  }
}

impl<A: Array> Debug for ArrayVecIterator<A>
where
  A::Item: Debug,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("ArrayVecIterator").field(&self.as_slice()).finish()
  }
}

impl<A: Array> IntoIterator for ArrayVec<A> {
  type Item = A::Item;
  type IntoIter = ArrayVecIterator<A>;
  #[inline(always)]
  fn into_iter(self) -> Self::IntoIter {
    ArrayVecIterator { base: 0, tail: self.len, data: self.data }
  }
}

impl<'a, A: Array> IntoIterator for &'a mut ArrayVec<A> {
  type Item = &'a mut A::Item;
  type IntoIter = core::slice::IterMut<'a, A::Item>;
  #[inline(always)]
  fn into_iter(self) -> Self::IntoIter {
    self.iter_mut()
  }
}

impl<'a, A: Array> IntoIterator for &'a ArrayVec<A> {
  type Item = &'a A::Item;
  type IntoIter = core::slice::Iter<'a, A::Item>;
  #[inline(always)]
  fn into_iter(self) -> Self::IntoIter {
    self.iter()
  }
}

impl<A: Array> PartialEq for ArrayVec<A>
where
  A::Item: PartialEq,
{
  #[inline]
  fn eq(&self, other: &Self) -> bool {
    self.as_slice().eq(other.as_slice())
  }
}
impl<A: Array> Eq for ArrayVec<A> where A::Item: Eq {}

impl<A: Array> PartialOrd for ArrayVec<A>
where
  A::Item: PartialOrd,
{
  #[inline]
  fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
    self.as_slice().partial_cmp(other.as_slice())
  }
}
impl<A: Array> Ord for ArrayVec<A>
where
  A::Item: Ord,
{
  #[inline]
  fn cmp(&self, other: &Self) -> core::cmp::Ordering {
    self.as_slice().cmp(other.as_slice())
  }
}

impl<A: Array> PartialEq<&A> for ArrayVec<A>
where
  A::Item: PartialEq,
{
  #[inline]
  fn eq(&self, other: &&A) -> bool {
    self.as_slice().eq(other.as_slice())
  }
}

impl<A: Array> PartialEq<&[A::Item]> for ArrayVec<A>
where
  A::Item: PartialEq,
{
  #[inline]
  fn eq(&self, other: &&[A::Item]) -> bool {
    self.as_slice().eq(*other)
  }
}

impl<A: Array> Hash for ArrayVec<A>
where
  A::Item: Hash,
{
  #[inline]
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.as_slice().hash(state)
  }
}

#[cfg(feature = "experimental_write_impl")]
impl<A: Array<Item = u8>> core::fmt::Write for ArrayVec<A> {
  fn write_str(&mut self, s: &str) -> core::fmt::Result {
    let my_len = self.len();
    let str_len = s.as_bytes().len();
    if my_len + str_len <= A::CAPACITY {
      let remainder = &mut self.data.as_slice_mut()[my_len..];
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

impl<A: Array> Binary for ArrayVec<A>
where
  A::Item: Binary,
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

impl<A: Array> Debug for ArrayVec<A>
where
  A::Item: Debug,
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

impl<A: Array> Display for ArrayVec<A>
where
  A::Item: Display,
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

impl<A: Array> LowerExp for ArrayVec<A>
where
  A::Item: LowerExp,
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

impl<A: Array> LowerHex for ArrayVec<A>
where
  A::Item: LowerHex,
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

impl<A: Array> Octal for ArrayVec<A>
where
  A::Item: Octal,
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

impl<A: Array> Pointer for ArrayVec<A>
where
  A::Item: Pointer,
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

impl<A: Array> UpperExp for ArrayVec<A>
where
  A::Item: UpperExp,
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

impl<A: Array> UpperHex for ArrayVec<A>
where
  A::Item: UpperHex,
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

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(all(feature = "alloc", feature = "rustc_1_57"))]
use alloc::collections::TryReserveError;

#[cfg(feature = "alloc")]
impl<A: Array> ArrayVec<A> {
  /// Drains all elements to a Vec, but reserves additional space
  /// ```
  /// # use tinyvec::*;
  /// let mut av = array_vec!([i32; 7] => 1, 2, 3);
  /// let v = av.drain_to_vec_and_reserve(10);
  /// assert_eq!(v, &[1, 2, 3]);
  /// assert_eq!(v.capacity(), 13);
  /// ```
  #[inline]
  pub fn drain_to_vec_and_reserve(&mut self, n: usize) -> Vec<A::Item> {
    let cap = n + self.len();
    let mut v = Vec::with_capacity(cap);
    let iter = self.iter_mut().map(core::mem::take);
    v.extend(iter);
    self.set_len(0);
    return v;
  }

  /// Tries to drain all elements to a Vec, but reserves additional space.
  ///
  /// # Errors
  ///
  /// If the allocator reports a failure, then an error is returned.
  ///
  /// ```
  /// # use tinyvec::*;
  /// let mut av = array_vec!([i32; 7] => 1, 2, 3);
  /// let v = av.try_drain_to_vec_and_reserve(10);
  /// assert!(matches!(v, Ok(_)));
  /// let v = v.unwrap();
  /// assert_eq!(v, &[1, 2, 3]);
  /// assert_eq!(v.capacity(), 13);
  /// ```
  #[inline]
  #[cfg(feature = "rustc_1_57")]
  pub fn try_drain_to_vec_and_reserve(
    &mut self, n: usize,
  ) -> Result<Vec<A::Item>, TryReserveError> {
    let cap = n + self.len();
    let mut v = Vec::new();
    v.try_reserve(cap)?;
    let iter = self.iter_mut().map(core::mem::take);
    v.extend(iter);
    self.set_len(0);
    return Ok(v);
  }

  /// Drains all elements to a Vec
  /// ```
  /// # use tinyvec::*;
  /// let mut av = array_vec!([i32; 7] => 1, 2, 3);
  /// let v = av.drain_to_vec();
  /// assert_eq!(v, &[1, 2, 3]);
  /// assert_eq!(v.capacity(), 3);
  /// ```
  #[inline]
  pub fn drain_to_vec(&mut self) -> Vec<A::Item> {
    self.drain_to_vec_and_reserve(0)
  }

  /// Tries to drain all elements to a Vec.
  ///
  /// # Errors
  ///
  /// If the allocator reports a failure, then an error is returned.
  ///
  /// ```
  /// # use tinyvec::*;
  /// let mut av = array_vec!([i32; 7] => 1, 2, 3);
  /// let v = av.try_drain_to_vec();
  /// assert!(matches!(v, Ok(_)));
  /// let v = v.unwrap();
  /// assert_eq!(v, &[1, 2, 3]);
  /// // Vec may reserve more than necessary in order to prevent more future allocations.
  /// assert!(v.capacity() >= 3);
  /// ```
  #[inline]
  #[cfg(feature = "rustc_1_57")]
  pub fn try_drain_to_vec(&mut self) -> Result<Vec<A::Item>, TryReserveError> {
    self.try_drain_to_vec_and_reserve(0)
  }
}

#[cfg(feature = "serde")]
struct ArrayVecVisitor<A: Array>(PhantomData<A>);

#[cfg(feature = "serde")]
impl<'de, A: Array> Visitor<'de> for ArrayVecVisitor<A>
where
  A::Item: Deserialize<'de>,
{
  type Value = ArrayVec<A>;

  fn expecting(
    &self, formatter: &mut core::fmt::Formatter,
  ) -> core::fmt::Result {
    formatter.write_str("a sequence")
  }

  fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
  where
    S: SeqAccess<'de>,
  {
    let mut new_arrayvec: ArrayVec<A> = Default::default();

    let mut idx = 0usize;
    while let Some(value) = seq.next_element()? {
      if new_arrayvec.len() >= new_arrayvec.capacity() {
        return Err(DeserializeError::invalid_length(idx, &self));
      }
      new_arrayvec.push(value);
      idx = idx + 1;
    }

    Ok(new_arrayvec)
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn retain_mut_empty_vec() {
    let mut av: ArrayVec<[i32; 4]> = ArrayVec::new();
    av.retain_mut(|&mut x| x % 2 == 0);
    assert_eq!(av.len(), 0);
  }

  #[test]
  fn retain_mut_all_elements() {
    let mut av: ArrayVec<[i32; 4]> = array_vec!([i32; 4] => 2, 4, 6, 8);
    av.retain_mut(|&mut x| x % 2 == 0);
    assert_eq!(av.len(), 4);
    assert_eq!(av.as_slice(), &[2, 4, 6, 8]);
  }

  #[test]
  fn retain_mut_some_elements() {
    let mut av: ArrayVec<[i32; 4]> = array_vec!([i32; 4] => 1, 2, 3, 4);
    av.retain_mut(|&mut x| x % 2 == 0);
    assert_eq!(av.len(), 2);
    assert_eq!(av.as_slice(), &[2, 4]);
  }

  #[test]
  fn retain_mut_no_elements() {
    let mut av: ArrayVec<[i32; 4]> = array_vec!([i32; 4] => 1, 3, 5, 7);
    av.retain_mut(|&mut x| x % 2 == 0);
    assert_eq!(av.len(), 0);
  }

  #[test]
  fn retain_mut_zero_capacity() {
    let mut av: ArrayVec<[i32; 0]> = ArrayVec::new();
    av.retain_mut(|&mut x| x % 2 == 0);
    assert_eq!(av.len(), 0);
  }
}
