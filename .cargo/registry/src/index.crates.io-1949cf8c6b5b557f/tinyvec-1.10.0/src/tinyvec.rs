use super::*;

use alloc::vec::{self, Vec};
use core::convert::TryFrom;
use tinyvec_macros::impl_mirrored;

#[cfg(feature = "rustc_1_57")]
use alloc::collections::TryReserveError;

#[cfg(feature = "serde")]
use core::marker::PhantomData;
#[cfg(feature = "serde")]
use serde::de::{Deserialize, Deserializer, SeqAccess, Visitor};
#[cfg(feature = "serde")]
use serde::ser::{Serialize, SerializeSeq, Serializer};

/// Helper to make a `TinyVec`.
///
/// You specify the backing array type, and optionally give all the elements you
/// want to initially place into the array.
///
/// ```rust
/// use tinyvec::*;
///
/// // The backing array type can be specified in the macro call
/// let empty_tv = tiny_vec!([u8; 16]);
/// let some_ints = tiny_vec!([i32; 4] => 1, 2, 3);
/// let many_ints = tiny_vec!([i32; 4] => 1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
///
/// // Or left to inference
/// let empty_tv: TinyVec<[u8; 16]> = tiny_vec!();
/// let some_ints: TinyVec<[i32; 4]> = tiny_vec!(1, 2, 3);
/// let many_ints: TinyVec<[i32; 4]> = tiny_vec!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
/// ```
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
macro_rules! tiny_vec {
  ($array_type:ty => $($elem:expr),* $(,)?) => {
    {
      // https://github.com/rust-lang/lang-team/issues/28
      const INVOKED_ELEM_COUNT: usize = 0 $( + { let _ = stringify!($elem); 1 })*;
      // If we have more `$elem` than the `CAPACITY` we will simply go directly
      // to constructing on the heap.
      match $crate::TinyVec::constructor_for_capacity(INVOKED_ELEM_COUNT) {
        $crate::TinyVecConstructor::Inline(f) => {
          f($crate::array_vec!($array_type => $($elem),*))
        }
        $crate::TinyVecConstructor::Heap(f) => {
          f(vec!($($elem),*))
        }
      }
    }
  };
  ($array_type:ty) => {
    $crate::TinyVec::<$array_type>::default()
  };
  ($($elem:expr),*) => {
    $crate::tiny_vec!(_ => $($elem),*)
  };
  ($elem:expr; $n:expr) => {
    $crate::TinyVec::from([$elem; $n])
  };
  () => {
    $crate::tiny_vec!(_)
  };
}

#[doc(hidden)] // Internal implementation details of `tiny_vec!`
pub enum TinyVecConstructor<A: Array> {
  Inline(fn(ArrayVec<A>) -> TinyVec<A>),
  Heap(fn(Vec<A::Item>) -> TinyVec<A>),
}

/// A vector that starts inline, but can automatically move to the heap.
///
/// * Requires the `alloc` feature
///
/// A `TinyVec` is either an Inline([`ArrayVec`](crate::ArrayVec::<A>)) or
/// Heap([`Vec`](https://doc.rust-lang.org/alloc/vec/struct.Vec.html)). The
/// interface for the type as a whole is a bunch of methods that just match on
/// the enum variant and then call the same method on the inner vec.
///
/// ## Construction
///
/// Because it's an enum, you can construct a `TinyVec` simply by making an
/// `ArrayVec` or `Vec` and then putting it into the enum.
///
/// There is also a macro
///
/// ```rust
/// # use tinyvec::*;
/// let empty_tv = tiny_vec!([u8; 16]);
/// let some_ints = tiny_vec!([i32; 4] => 1, 2, 3);
/// ```
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub enum TinyVec<A: Array> {
  #[allow(missing_docs)]
  Inline(ArrayVec<A>),
  #[allow(missing_docs)]
  Heap(Vec<A::Item>),
}

impl<A> Clone for TinyVec<A>
where
  A: Array + Clone,
  A::Item: Clone,
{
  #[inline]
  fn clone(&self) -> Self {
    match self {
      TinyVec::Heap(v) => TinyVec::Heap(v.clone()),
      TinyVec::Inline(v) => TinyVec::Inline(v.clone()),
    }
  }

  #[inline]
  fn clone_from(&mut self, o: &Self) {
    if o.len() > self.len() {
      self.reserve(o.len() - self.len());
    } else {
      self.truncate(o.len());
    }
    let (start, end) = o.split_at(self.len());
    for (dst, src) in self.iter_mut().zip(start) {
      dst.clone_from(src);
    }
    self.extend_from_slice(end);
  }
}

impl<A: Array> Default for TinyVec<A> {
  #[inline]
  fn default() -> Self {
    TinyVec::Inline(ArrayVec::default())
  }
}

impl<A: Array> Deref for TinyVec<A> {
  type Target = [A::Item];

  impl_mirrored! {
    type Mirror = TinyVec;
    #[inline(always)]
    #[must_use]
    fn deref(self: &Self) -> &Self::Target;
  }
}

impl<A: Array> DerefMut for TinyVec<A> {
  impl_mirrored! {
    type Mirror = TinyVec;
    #[inline(always)]
    #[must_use]
    fn deref_mut(self: &mut Self) -> &mut Self::Target;
  }
}

impl<A: Array, I: SliceIndex<[A::Item]>> Index<I> for TinyVec<A> {
  type Output = <I as SliceIndex<[A::Item]>>::Output;
  #[inline(always)]
  fn index(&self, index: I) -> &Self::Output {
    &self.deref()[index]
  }
}

impl<A: Array, I: SliceIndex<[A::Item]>> IndexMut<I> for TinyVec<A> {
  #[inline(always)]
  fn index_mut(&mut self, index: I) -> &mut Self::Output {
    &mut self.deref_mut()[index]
  }
}

#[cfg(feature = "std")]
#[cfg_attr(docs_rs, doc(cfg(feature = "std")))]
impl<A: Array<Item = u8>> std::io::Write for TinyVec<A> {
  #[inline(always)]
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    self.extend_from_slice(buf);
    Ok(buf.len())
  }

  #[inline(always)]
  fn flush(&mut self) -> std::io::Result<()> {
    Ok(())
  }
}

#[cfg(feature = "serde")]
#[cfg_attr(docs_rs, doc(cfg(feature = "serde")))]
impl<A: Array> Serialize for TinyVec<A>
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
impl<'de, A: Array> Deserialize<'de> for TinyVec<A>
where
  A::Item: Deserialize<'de>,
{
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_seq(TinyVecVisitor(PhantomData))
  }
}

#[cfg(feature = "borsh")]
#[cfg_attr(docs_rs, doc(cfg(feature = "borsh")))]
impl<A: Array> borsh::BorshSerialize for TinyVec<A>
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
impl<A: Array> borsh::BorshDeserialize for TinyVec<A>
where
  <A as Array>::Item: borsh::BorshDeserialize,
{
  fn deserialize_reader<R: borsh::io::Read>(
    reader: &mut R,
  ) -> borsh::io::Result<Self> {
    let len = <usize as borsh::BorshDeserialize>::deserialize_reader(reader)?;
    let mut new_tinyvec = Self::with_capacity(len);

    for _ in 0..len {
      new_tinyvec.push(
        <<A as Array>::Item as borsh::BorshDeserialize>::deserialize_reader(
          reader,
        )?,
      )
    }

    Ok(new_tinyvec)
  }
}

#[cfg(feature = "arbitrary")]
#[cfg_attr(docs_rs, doc(cfg(feature = "arbitrary")))]
impl<'a, A> arbitrary::Arbitrary<'a> for TinyVec<A>
where
  A: Array,
  A::Item: arbitrary::Arbitrary<'a>,
{
  fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
    let v = Vec::arbitrary(u)?;
    let mut tv = TinyVec::Heap(v);
    tv.shrink_to_fit();
    Ok(tv)
  }
}

impl<A: Array> TinyVec<A> {
  /// Returns whether elements are on heap
  #[inline(always)]
  #[must_use]
  pub fn is_heap(&self) -> bool {
    match self {
      TinyVec::Heap(_) => true,
      TinyVec::Inline(_) => false,
    }
  }
  /// Returns whether elements are on stack
  #[inline(always)]
  #[must_use]
  pub fn is_inline(&self) -> bool {
    !self.is_heap()
  }

  /// Shrinks the capacity of the vector as much as possible.\
  /// It is inlined if length is less than `A::CAPACITY`.
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 2] => 1, 2, 3);
  /// assert!(tv.is_heap());
  /// let _ = tv.pop();
  /// assert!(tv.is_heap());
  /// tv.shrink_to_fit();
  /// assert!(tv.is_inline());
  /// ```
  #[inline]
  pub fn shrink_to_fit(&mut self) {
    let vec = match self {
      TinyVec::Inline(_) => return,
      TinyVec::Heap(h) => h,
    };

    if vec.len() > A::CAPACITY {
      return vec.shrink_to_fit();
    }

    let moved_vec = core::mem::take(vec);

    let mut av = ArrayVec::default();
    let mut rest = av.fill(moved_vec);
    debug_assert!(rest.next().is_none());
    *self = TinyVec::Inline(av);
  }

  /// Moves the content of the TinyVec to the heap, if it's inline.
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3);
  /// assert!(tv.is_inline());
  /// tv.move_to_the_heap();
  /// assert!(tv.is_heap());
  /// ```
  #[allow(clippy::missing_inline_in_public_items)]
  pub fn move_to_the_heap(&mut self) {
    let arr = match self {
      TinyVec::Heap(_) => return,
      TinyVec::Inline(a) => a,
    };

    let v = arr.drain_to_vec();
    *self = TinyVec::Heap(v);
  }

  /// Tries to move the content of the TinyVec to the heap, if it's inline.
  ///
  /// # Errors
  ///
  /// If the allocator reports a failure, then an error is returned and the
  /// content is kept on the stack.
  ///
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3);
  /// assert!(tv.is_inline());
  /// assert_eq!(Ok(()), tv.try_move_to_the_heap());
  /// assert!(tv.is_heap());
  /// ```
  #[inline]
  #[cfg(feature = "rustc_1_57")]
  pub fn try_move_to_the_heap(&mut self) -> Result<(), TryReserveError> {
    let arr = match self {
      TinyVec::Heap(_) => return Ok(()),
      TinyVec::Inline(a) => a,
    };

    let v = arr.try_drain_to_vec()?;
    *self = TinyVec::Heap(v);
    return Ok(());
  }

  /// If TinyVec is inline, moves the content of it to the heap.
  /// Also reserves additional space.
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3);
  /// assert!(tv.is_inline());
  /// tv.move_to_the_heap_and_reserve(32);
  /// assert!(tv.is_heap());
  /// assert!(tv.capacity() >= 35);
  /// ```
  #[inline]
  pub fn move_to_the_heap_and_reserve(&mut self, n: usize) {
    let arr = match self {
      TinyVec::Heap(h) => return h.reserve(n),
      TinyVec::Inline(a) => a,
    };

    let v = arr.drain_to_vec_and_reserve(n);
    *self = TinyVec::Heap(v);
  }

  /// If TinyVec is inline, try to move the content of it to the heap.
  /// Also reserves additional space.
  ///
  /// # Errors
  ///
  /// If the allocator reports a failure, then an error is returned.
  ///
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3);
  /// assert!(tv.is_inline());
  /// assert_eq!(Ok(()), tv.try_move_to_the_heap_and_reserve(32));
  /// assert!(tv.is_heap());
  /// assert!(tv.capacity() >= 35);
  /// ```
  #[inline]
  #[cfg(feature = "rustc_1_57")]
  pub fn try_move_to_the_heap_and_reserve(
    &mut self, n: usize,
  ) -> Result<(), TryReserveError> {
    let arr = match self {
      TinyVec::Heap(h) => return h.try_reserve(n),
      TinyVec::Inline(a) => a,
    };

    let v = arr.try_drain_to_vec_and_reserve(n)?;
    *self = TinyVec::Heap(v);
    return Ok(());
  }

  /// Reserves additional space.
  /// Moves to the heap if array can't hold `n` more items
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3, 4);
  /// assert!(tv.is_inline());
  /// tv.reserve(1);
  /// assert!(tv.is_heap());
  /// assert!(tv.capacity() >= 5);
  /// ```
  #[inline]
  pub fn reserve(&mut self, n: usize) {
    let arr = match self {
      TinyVec::Heap(h) => return h.reserve(n),
      TinyVec::Inline(a) => a,
    };

    if n > arr.capacity() - arr.len() {
      let v = arr.drain_to_vec_and_reserve(n);
      *self = TinyVec::Heap(v);
    }

    /* In this place array has enough place, so no work is needed more */
    return;
  }

  /// Tries to reserve additional space.
  /// Moves to the heap if array can't hold `n` more items.
  ///
  /// # Errors
  ///
  /// If the allocator reports a failure, then an error is returned.
  ///
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3, 4);
  /// assert!(tv.is_inline());
  /// assert_eq!(Ok(()), tv.try_reserve(1));
  /// assert!(tv.is_heap());
  /// assert!(tv.capacity() >= 5);
  /// ```
  #[inline]
  #[cfg(feature = "rustc_1_57")]
  pub fn try_reserve(&mut self, n: usize) -> Result<(), TryReserveError> {
    let arr = match self {
      TinyVec::Heap(h) => return h.try_reserve(n),
      TinyVec::Inline(a) => a,
    };

    if n > arr.capacity() - arr.len() {
      let v = arr.try_drain_to_vec_and_reserve(n)?;
      *self = TinyVec::Heap(v);
    }

    /* In this place array has enough place, so no work is needed more */
    return Ok(());
  }

  /// Reserves additional space.
  /// Moves to the heap if array can't hold `n` more items
  ///
  /// From [Vec::reserve_exact](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.reserve_exact)
  /// ```text
  /// Note that the allocator may give the collection more space than it requests.
  /// Therefore, capacity can not be relied upon to be precisely minimal.
  /// Prefer `reserve` if future insertions are expected.
  /// ```
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3, 4);
  /// assert!(tv.is_inline());
  /// tv.reserve_exact(1);
  /// assert!(tv.is_heap());
  /// assert!(tv.capacity() >= 5);
  /// ```
  #[inline]
  pub fn reserve_exact(&mut self, n: usize) {
    let arr = match self {
      TinyVec::Heap(h) => return h.reserve_exact(n),
      TinyVec::Inline(a) => a,
    };

    if n > arr.capacity() - arr.len() {
      let v = arr.drain_to_vec_and_reserve(n);
      *self = TinyVec::Heap(v);
    }

    /* In this place array has enough place, so no work is needed more */
    return;
  }

  /// Tries to reserve additional space.
  /// Moves to the heap if array can't hold `n` more items
  ///
  /// # Errors
  ///
  /// If the allocator reports a failure, then an error is returned.
  ///
  /// From [Vec::try_reserve_exact](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.try_reserve_exact)
  /// ```text
  /// Note that the allocator may give the collection more space than it requests.
  /// Therefore, capacity can not be relied upon to be precisely minimal.
  /// Prefer `reserve` if future insertions are expected.
  /// ```
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3, 4);
  /// assert!(tv.is_inline());
  /// assert_eq!(Ok(()), tv.try_reserve_exact(1));
  /// assert!(tv.is_heap());
  /// assert!(tv.capacity() >= 5);
  /// ```
  #[inline]
  #[cfg(feature = "rustc_1_57")]
  pub fn try_reserve_exact(&mut self, n: usize) -> Result<(), TryReserveError> {
    let arr = match self {
      TinyVec::Heap(h) => return h.try_reserve_exact(n),
      TinyVec::Inline(a) => a,
    };

    if n > arr.capacity() - arr.len() {
      let v = arr.try_drain_to_vec_and_reserve(n)?;
      *self = TinyVec::Heap(v);
    }

    /* In this place array has enough place, so no work is needed more */
    return Ok(());
  }

  /// Makes a new TinyVec with _at least_ the given capacity.
  ///
  /// If the requested capacity is less than or equal to the array capacity you
  /// get an inline vec. If it's greater than you get a heap vec.
  /// ```
  /// # use tinyvec::*;
  /// let t = TinyVec::<[u8; 10]>::with_capacity(5);
  /// assert!(t.is_inline());
  /// assert!(t.capacity() >= 5);
  ///
  /// let t = TinyVec::<[u8; 10]>::with_capacity(20);
  /// assert!(t.is_heap());
  /// assert!(t.capacity() >= 20);
  /// ```
  #[inline]
  #[must_use]
  pub fn with_capacity(cap: usize) -> Self {
    if cap <= A::CAPACITY {
      TinyVec::Inline(ArrayVec::default())
    } else {
      TinyVec::Heap(Vec::with_capacity(cap))
    }
  }

  /// Converts a `TinyVec<[T; N]>` into a `Box<[T]>`.
  ///
  /// - For `TinyVec::Heap(Vec<T>)`, it takes the `Vec<T>` and converts it into
  ///   a `Box<[T]>` without heap reallocation.
  /// - For `TinyVec::Inline(inner_data)`, it first converts the `inner_data` to
  ///   `Vec<T>`, then into a `Box<[T]>`. Requiring only a single heap
  ///   allocation.
  ///
  /// ## Example
  ///
  /// ```
  /// use core::mem::size_of_val as mem_size_of;
  /// use tinyvec::TinyVec;
  ///
  /// // Initialize TinyVec with 256 elements (exceeding inline capacity)
  /// let v: TinyVec<[_; 128]> = (0u8..=255).collect();
  ///
  /// assert!(v.is_heap());
  /// assert_eq!(mem_size_of(&v), 136); // mem size of TinyVec<[u8; N]>: N+8
  /// assert_eq!(v.len(), 256);
  ///
  /// let boxed = v.into_boxed_slice();
  /// assert_eq!(mem_size_of(&boxed), 16); // mem size of Box<[u8]>: 16 bytes (fat pointer)
  /// assert_eq!(boxed.len(), 256);
  /// ```
  #[inline]
  #[must_use]
  pub fn into_boxed_slice(self) -> alloc::boxed::Box<[A::Item]> {
    self.into_vec().into_boxed_slice()
  }

  /// Converts a `TinyVec<[T; N]>` into a `Vec<T>`.
  ///
  /// `v.into_vec()` is equivalent to `Into::<Vec<_>>::into(v)`.
  ///
  /// - For `TinyVec::Inline(_)`, `.into_vec()` **does not** offer a performance
  ///   advantage over `.to_vec()`.
  /// - For `TinyVec::Heap(vec_data)`, `.into_vec()` will take `vec_data`
  ///   without heap reallocation.
  ///
  /// ## Example
  ///
  /// ```
  /// use tinyvec::TinyVec;
  ///
  /// let v = TinyVec::from([0u8; 8]);
  /// let v2 = v.clone();
  ///
  /// let vec = v.into_vec();
  /// let vec2: Vec<_> = v2.into();
  ///
  /// assert_eq!(vec, vec2);
  /// ```
  #[inline]
  #[must_use]
  pub fn into_vec(self) -> Vec<A::Item> {
    self.into()
  }
}

impl<A: Array> TinyVec<A> {
  /// Move all values from `other` into this vec.
  #[inline]
  pub fn append(&mut self, other: &mut Self) {
    self.reserve(other.len());

    /* Doing append should be faster, because it is effectively a memcpy */
    match (self, other) {
      (TinyVec::Heap(sh), TinyVec::Heap(oh)) => sh.append(oh),
      (TinyVec::Inline(a), TinyVec::Heap(h)) => a.extend(h.drain(..)),
      (ref mut this, TinyVec::Inline(arr)) => this.extend(arr.drain(..)),
    }
  }

  impl_mirrored! {
    type Mirror = TinyVec;

    /// Remove an element, swapping the end of the vec into its place.
    ///
    /// ## Panics
    /// * If the index is out of bounds.
    ///
    /// ## Example
    /// ```rust
    /// use tinyvec::*;
    /// let mut tv = tiny_vec!([&str; 4] => "foo", "bar", "quack", "zap");
    ///
    /// assert_eq!(tv.swap_remove(1), "bar");
    /// assert_eq!(tv.as_slice(), &["foo", "zap", "quack"][..]);
    ///
    /// assert_eq!(tv.swap_remove(0), "foo");
    /// assert_eq!(tv.as_slice(), &["quack", "zap"][..]);
    /// ```
    #[inline]
    pub fn swap_remove(self: &mut Self, index: usize) -> A::Item;

    /// Remove and return the last element of the vec, if there is one.
    ///
    /// ## Failure
    /// * If the vec is empty you get `None`.
    #[inline]
    pub fn pop(self: &mut Self) -> Option<A::Item>;

    /// Removes the item at `index`, shifting all others down by one index.
    ///
    /// Returns the removed element.
    ///
    /// ## Panics
    ///
    /// If the index is out of bounds.
    ///
    /// ## Example
    ///
    /// ```rust
    /// use tinyvec::*;
    /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3);
    /// assert_eq!(tv.remove(1), 2);
    /// assert_eq!(tv.as_slice(), &[1, 3][..]);
    /// ```
    #[inline]
    pub fn remove(self: &mut Self, index: usize) -> A::Item;

    /// The length of the vec (in elements).
    #[inline(always)]
    #[must_use]
    pub fn len(self: &Self) -> usize;

    /// The capacity of the `TinyVec`.
    ///
    /// When not heap allocated this is fixed based on the array type.
    /// Otherwise its the result of the underlying Vec::capacity.
    #[inline(always)]
    #[must_use]
    pub fn capacity(self: &Self) -> usize;

    /// Reduces the vec's length to the given value.
    ///
    /// If the vec is already shorter than the input, nothing happens.
    #[inline]
    pub fn truncate(self: &mut Self, new_len: usize);

    /// A mutable pointer to the backing array.
    ///
    /// ## Safety
    ///
    /// This pointer has provenance over the _entire_ backing array/buffer.
    #[inline(always)]
    #[must_use]
    pub fn as_mut_ptr(self: &mut Self) -> *mut A::Item;

    /// A const pointer to the backing array.
    ///
    /// ## Safety
    ///
    /// This pointer has provenance over the _entire_ backing array/buffer.
    #[inline(always)]
    #[must_use]
    pub fn as_ptr(self: &Self) -> *const A::Item;
  }

  /// Walk the vec and keep only the elements that pass the predicate given.
  ///
  /// ## Example
  ///
  /// ```rust
  /// use tinyvec::*;
  ///
  /// let mut tv = tiny_vec!([i32; 10] => 1, 2, 3, 4);
  /// tv.retain(|&x| x % 2 == 0);
  /// assert_eq!(tv.as_slice(), &[2, 4][..]);
  /// ```
  #[inline]
  pub fn retain<F: FnMut(&A::Item) -> bool>(&mut self, acceptable: F) {
    match self {
      TinyVec::Inline(i) => i.retain(acceptable),
      TinyVec::Heap(h) => h.retain(acceptable),
    }
  }

  /// Walk the vec and keep only the elements that pass the predicate given,
  /// having the opportunity to modify the elements at the same time.
  ///
  /// ## Example
  ///
  /// ```rust
  /// use tinyvec::*;
  ///
  /// let mut tv = tiny_vec!([i32; 10] => 1, 2, 3, 4);
  /// tv.retain_mut(|x| if *x % 2 == 0 { *x *= 2; true } else { false });
  /// assert_eq!(tv.as_slice(), &[4, 8][..]);
  /// ```
  #[inline]
  #[cfg(feature = "rustc_1_61")]
  pub fn retain_mut<F: FnMut(&mut A::Item) -> bool>(&mut self, acceptable: F) {
    match self {
      TinyVec::Inline(i) => i.retain_mut(acceptable),
      TinyVec::Heap(h) => h.retain_mut(acceptable),
    }
  }

  /// Helper for getting the mut slice.
  #[inline(always)]
  #[must_use]
  pub fn as_mut_slice(&mut self) -> &mut [A::Item] {
    self.deref_mut()
  }

  /// Helper for getting the shared slice.
  #[inline(always)]
  #[must_use]
  pub fn as_slice(&self) -> &[A::Item] {
    self.deref()
  }

  /// Removes all elements from the vec.
  #[inline(always)]
  pub fn clear(&mut self) {
    self.truncate(0)
  }

  /// De-duplicates the vec.
  #[cfg(feature = "nightly_slice_partition_dedup")]
  #[inline(always)]
  pub fn dedup(&mut self)
  where
    A::Item: PartialEq,
  {
    self.dedup_by(|a, b| a == b)
  }

  /// De-duplicates the vec according to the predicate given.
  #[cfg(feature = "nightly_slice_partition_dedup")]
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
  #[cfg(feature = "nightly_slice_partition_dedup")]
  #[inline(always)]
  pub fn dedup_by_key<F, K>(&mut self, mut key: F)
  where
    F: FnMut(&mut A::Item) -> K,
    K: PartialEq,
  {
    self.dedup_by(|a, b| key(a) == key(b))
  }

  /// Creates a draining iterator that removes the specified range in the vector
  /// and yields the removed items.
  ///
  /// **Note: This method has significant performance issues compared to
  /// matching on the TinyVec and then calling drain on the Inline or Heap value
  /// inside. The draining iterator has to branch on every single access. It is
  /// provided for simplicity and compatibility only.**
  ///
  /// ## Panics
  /// * If the start is greater than the end
  /// * If the end is past the edge of the vec.
  ///
  /// ## Example
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3);
  /// let tv2: TinyVec<[i32; 4]> = tv.drain(1..).collect();
  /// assert_eq!(tv.as_slice(), &[1][..]);
  /// assert_eq!(tv2.as_slice(), &[2, 3][..]);
  ///
  /// tv.drain(..);
  /// assert_eq!(tv.as_slice(), &[]);
  /// ```
  #[inline]
  pub fn drain<R: RangeBounds<usize>>(
    &mut self, range: R,
  ) -> TinyVecDrain<'_, A> {
    match self {
      TinyVec::Inline(i) => TinyVecDrain::Inline(i.drain(range)),
      TinyVec::Heap(h) => TinyVecDrain::Heap(h.drain(range)),
    }
  }

  /// Clone each element of the slice into this vec.
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2);
  /// tv.extend_from_slice(&[3, 4]);
  /// assert_eq!(tv.as_slice(), [1, 2, 3, 4]);
  /// ```
  #[inline]
  pub fn extend_from_slice(&mut self, sli: &[A::Item])
  where
    A::Item: Clone,
  {
    self.reserve(sli.len());
    match self {
      TinyVec::Inline(a) => a.extend_from_slice(sli),
      TinyVec::Heap(h) => h.extend_from_slice(sli),
    }
  }

  /// Wraps up an array and uses the given length as the initial length.
  ///
  /// Note that the `From` impl for arrays assumes the full length is used.
  ///
  /// ## Panics
  ///
  /// The length must be less than or equal to the capacity of the array.
  #[inline]
  #[must_use]
  #[allow(clippy::match_wild_err_arm)]
  pub fn from_array_len(data: A, len: usize) -> Self {
    match Self::try_from_array_len(data, len) {
      Ok(out) => out,
      Err(_) => {
        panic!("TinyVec: length {} exceeds capacity {}!", len, A::CAPACITY)
      }
    }
  }

  /// This is an internal implementation detail of the `tiny_vec!` macro, and
  /// using it other than from that macro is not supported by this crate's
  /// SemVer guarantee.
  #[inline(always)]
  #[doc(hidden)]
  pub fn constructor_for_capacity(cap: usize) -> TinyVecConstructor<A> {
    if cap <= A::CAPACITY {
      TinyVecConstructor::Inline(TinyVec::Inline)
    } else {
      TinyVecConstructor::Heap(TinyVec::Heap)
    }
  }

  /// Inserts an item at the position given, moving all following elements +1
  /// index.
  ///
  /// ## Panics
  /// * If `index` > `len`
  ///
  /// ## Example
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 10] => 1, 2, 3);
  /// tv.insert(1, 4);
  /// assert_eq!(tv.as_slice(), &[1, 4, 2, 3]);
  /// tv.insert(4, 5);
  /// assert_eq!(tv.as_slice(), &[1, 4, 2, 3, 5]);
  /// ```
  #[inline]
  pub fn insert(&mut self, index: usize, item: A::Item) {
    assert!(
      index <= self.len(),
      "insertion index (is {}) should be <= len (is {})",
      index,
      self.len()
    );

    let arr = match self {
      TinyVec::Heap(v) => return v.insert(index, item),
      TinyVec::Inline(a) => a,
    };

    if let Some(x) = arr.try_insert(index, item) {
      let mut v = Vec::with_capacity(arr.len() * 2);
      let mut it = arr.iter_mut().map(core::mem::take);
      v.extend(it.by_ref().take(index));
      v.push(x);
      v.extend(it);
      *self = TinyVec::Heap(v);
    }
  }

  /// If the vec is empty.
  #[inline(always)]
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Makes a new, empty vec.
  #[inline(always)]
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Place an element onto the end of the vec.
  #[inline]
  pub fn push(&mut self, val: A::Item) {
    // The code path for moving the inline contents to the heap produces a lot
    // of instructions, but we have a strong guarantee that this is a cold
    // path. LLVM doesn't know this, inlines it, and this tends to cause a
    // cascade of other bad inlining decisions because the body of push looks
    // huge even though nearly every call executes the same few instructions.
    //
    // Moving the logic out of line with #[cold] causes the hot code to  be
    // inlined together, and we take the extra cost of a function call only
    // in rare cases.
    #[cold]
    fn drain_to_heap_and_push<A: Array>(
      arr: &mut ArrayVec<A>, val: A::Item,
    ) -> TinyVec<A> {
      /* Make the Vec twice the size to amortize the cost of draining */
      let mut v = arr.drain_to_vec_and_reserve(arr.len());
      v.push(val);
      TinyVec::Heap(v)
    }

    match self {
      TinyVec::Heap(v) => v.push(val),
      TinyVec::Inline(arr) => {
        if let Some(x) = arr.try_push(val) {
          *self = drain_to_heap_and_push(arr, x);
        }
      }
    }
  }

  /// Resize the vec to the new length.
  ///
  /// If it needs to be longer, it's filled with clones of the provided value.
  /// If it needs to be shorter, it's truncated.
  ///
  /// ## Example
  ///
  /// ```rust
  /// use tinyvec::*;
  ///
  /// let mut tv = tiny_vec!([&str; 10] => "hello");
  /// tv.resize(3, "world");
  /// assert_eq!(tv.as_slice(), &["hello", "world", "world"][..]);
  ///
  /// let mut tv = tiny_vec!([i32; 10] => 1, 2, 3, 4);
  /// tv.resize(2, 0);
  /// assert_eq!(tv.as_slice(), &[1, 2][..]);
  /// ```
  #[inline]
  pub fn resize(&mut self, new_len: usize, new_val: A::Item)
  where
    A::Item: Clone,
  {
    self.resize_with(new_len, || new_val.clone());
  }

  /// Resize the vec to the new length.
  ///
  /// If it needs to be longer, it's filled with repeated calls to the provided
  /// function. If it needs to be shorter, it's truncated.
  ///
  /// ## Example
  ///
  /// ```rust
  /// use tinyvec::*;
  ///
  /// let mut tv = tiny_vec!([i32; 3] => 1, 2, 3);
  /// tv.resize_with(5, Default::default);
  /// assert_eq!(tv.as_slice(), &[1, 2, 3, 0, 0][..]);
  ///
  /// let mut tv = tiny_vec!([i32; 2]);
  /// let mut p = 1;
  /// tv.resize_with(4, || {
  ///   p *= 2;
  ///   p
  /// });
  /// assert_eq!(tv.as_slice(), &[2, 4, 8, 16][..]);
  /// ```
  #[inline]
  pub fn resize_with<F: FnMut() -> A::Item>(&mut self, new_len: usize, f: F) {
    match new_len.checked_sub(self.len()) {
      None => return self.truncate(new_len),
      Some(n) => self.reserve(n),
    }

    match self {
      TinyVec::Inline(a) => a.resize_with(new_len, f),
      TinyVec::Heap(v) => v.resize_with(new_len, f),
    }
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
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3);
  /// let tv2 = tv.split_off(1);
  /// assert_eq!(tv.as_slice(), &[1][..]);
  /// assert_eq!(tv2.as_slice(), &[2, 3][..]);
  /// ```
  #[inline]
  pub fn split_off(&mut self, at: usize) -> Self {
    match self {
      TinyVec::Inline(a) => TinyVec::Inline(a.split_off(at)),
      TinyVec::Heap(v) => TinyVec::Heap(v.split_off(at)),
    }
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
  ///
  /// ## Example
  /// ```rust
  /// use tinyvec::*;
  /// let mut tv = tiny_vec!([i32; 4] => 1, 2, 3);
  /// let tv2: TinyVec<[i32; 4]> = tv.splice(1.., 4..=6).collect();
  /// assert_eq!(tv.as_slice(), &[1, 4, 5, 6][..]);
  /// assert_eq!(tv2.as_slice(), &[2, 3][..]);
  ///
  /// tv.splice(.., None);
  /// assert_eq!(tv.as_slice(), &[]);
  /// ```
  #[inline]
  pub fn splice<R, I>(
    &mut self, range: R, replacement: I,
  ) -> TinyVecSplice<'_, A, core::iter::Fuse<I::IntoIter>>
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
      "TinyVec::splice> Illegal range, {} to {}",
      start,
      end
    );
    assert!(
      end <= self.len(),
      "TinyVec::splice> Range ends at {} but length is only {}!",
      end,
      self.len()
    );

    TinyVecSplice {
      removal_start: start,
      removal_end: end,
      parent: self,
      replacement: replacement.into_iter().fuse(),
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
  pub fn try_from_array_len(data: A, len: usize) -> Result<Self, A> {
    let arr = ArrayVec::try_from_array_len(data, len)?;
    Ok(TinyVec::Inline(arr))
  }
}

/// Draining iterator for `TinyVecDrain`
///
/// See [`TinyVecDrain::drain`](TinyVecDrain::<A>::drain)
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub enum TinyVecDrain<'p, A: Array> {
  #[allow(missing_docs)]
  Inline(ArrayVecDrain<'p, A::Item>),
  #[allow(missing_docs)]
  Heap(vec::Drain<'p, A::Item>),
}

impl<'p, A: Array> Iterator for TinyVecDrain<'p, A> {
  type Item = A::Item;

  impl_mirrored! {
    type Mirror = TinyVecDrain;

    #[inline]
    fn next(self: &mut Self) -> Option<Self::Item>;
    #[inline]
    fn nth(self: &mut Self, n: usize) -> Option<Self::Item>;
    #[inline]
    fn size_hint(self: &Self) -> (usize, Option<usize>);
    #[inline]
    fn last(self: Self) -> Option<Self::Item>;
    #[inline]
    fn count(self: Self) -> usize;
  }

  #[inline]
  fn for_each<F: FnMut(Self::Item)>(self, f: F) {
    match self {
      TinyVecDrain::Inline(i) => i.for_each(f),
      TinyVecDrain::Heap(h) => h.for_each(f),
    }
  }
}

impl<'p, A: Array> DoubleEndedIterator for TinyVecDrain<'p, A> {
  impl_mirrored! {
    type Mirror = TinyVecDrain;

    #[inline]
    fn next_back(self: &mut Self) -> Option<Self::Item>;

    #[inline]
    fn nth_back(self: &mut Self, n: usize) -> Option<Self::Item>;
  }
}

/// Splicing iterator for `TinyVec`
/// See [`TinyVec::splice`](TinyVec::<A>::splice)
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct TinyVecSplice<'p, A: Array, I: Iterator<Item = A::Item>> {
  parent: &'p mut TinyVec<A>,
  removal_start: usize,
  removal_end: usize,
  replacement: I,
}

impl<'p, A, I> Iterator for TinyVecSplice<'p, A, I>
where
  A: Array,
  I: Iterator<Item = A::Item>,
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

impl<'p, A, I> ExactSizeIterator for TinyVecSplice<'p, A, I>
where
  A: Array,
  I: Iterator<Item = A::Item>,
{
  #[inline]
  fn len(&self) -> usize {
    self.removal_end - self.removal_start
  }
}

impl<'p, A, I> FusedIterator for TinyVecSplice<'p, A, I>
where
  A: Array,
  I: Iterator<Item = A::Item>,
{
}

impl<'p, A, I> DoubleEndedIterator for TinyVecSplice<'p, A, I>
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
  for TinyVecSplice<'p, A, I>
{
  #[inline]
  fn drop(&mut self) {
    for _ in self.by_ref() {}

    let (lower_bound, _) = self.replacement.size_hint();
    self.parent.reserve(lower_bound);

    for replacement in self.replacement.by_ref() {
      self.parent.insert(self.removal_end, replacement);
      self.removal_end += 1;
    }
  }
}

impl<A: Array> AsMut<[A::Item]> for TinyVec<A> {
  #[inline(always)]
  fn as_mut(&mut self) -> &mut [A::Item] {
    &mut *self
  }
}

impl<A: Array> AsRef<[A::Item]> for TinyVec<A> {
  #[inline(always)]
  fn as_ref(&self) -> &[A::Item] {
    &*self
  }
}

impl<A: Array> Borrow<[A::Item]> for TinyVec<A> {
  #[inline(always)]
  fn borrow(&self) -> &[A::Item] {
    &*self
  }
}

impl<A: Array> BorrowMut<[A::Item]> for TinyVec<A> {
  #[inline(always)]
  fn borrow_mut(&mut self) -> &mut [A::Item] {
    &mut *self
  }
}

impl<A: Array> Extend<A::Item> for TinyVec<A> {
  #[inline]
  fn extend<T: IntoIterator<Item = A::Item>>(&mut self, iter: T) {
    let iter = iter.into_iter();
    let (lower_bound, _) = iter.size_hint();
    self.reserve(lower_bound);

    let a = match self {
      TinyVec::Heap(h) => return h.extend(iter),
      TinyVec::Inline(a) => a,
    };

    let mut iter = a.fill(iter);
    let maybe = iter.next();

    let surely = match maybe {
      Some(x) => x,
      None => return,
    };

    let mut v = a.drain_to_vec_and_reserve(a.len());
    v.push(surely);
    v.extend(iter);
    *self = TinyVec::Heap(v);
  }
}

impl<A: Array> From<ArrayVec<A>> for TinyVec<A> {
  #[inline(always)]
  fn from(arr: ArrayVec<A>) -> Self {
    TinyVec::Inline(arr)
  }
}

impl<A: Array> From<A> for TinyVec<A> {
  #[inline]
  fn from(array: A) -> Self {
    TinyVec::Inline(ArrayVec::from(array))
  }
}

impl<T, A> From<&'_ [T]> for TinyVec<A>
where
  T: Clone + Default,
  A: Array<Item = T>,
{
  #[inline]
  fn from(slice: &[T]) -> Self {
    if let Ok(arr) = ArrayVec::try_from(slice) {
      TinyVec::Inline(arr)
    } else {
      TinyVec::Heap(slice.into())
    }
  }
}

impl<T, A> From<&'_ mut [T]> for TinyVec<A>
where
  T: Clone + Default,
  A: Array<Item = T>,
{
  #[inline]
  fn from(slice: &mut [T]) -> Self {
    Self::from(&*slice)
  }
}

impl<A: Array> FromIterator<A::Item> for TinyVec<A> {
  #[inline]
  fn from_iter<T: IntoIterator<Item = A::Item>>(iter: T) -> Self {
    let mut av = Self::default();
    av.extend(iter);
    av
  }
}

impl<A: Array> Into<Vec<A::Item>> for TinyVec<A> {
  /// Converts a `TinyVec` into a `Vec`.
  ///
  /// ## Examples
  ///
  /// ### Inline to Vec
  ///
  /// For `TinyVec::Inline(_)`,
  ///   `.into()` **does not** offer a performance advantage over `.to_vec()`.
  ///
  /// ```
  /// use core::mem::size_of_val as mem_size_of;
  /// use tinyvec::TinyVec;
  ///
  /// let v = TinyVec::from([0u8; 128]);
  /// assert_eq!(mem_size_of(&v), 136);
  ///
  /// let vec: Vec<_> = v.into();
  /// assert_eq!(mem_size_of(&vec), 24);
  /// ```
  ///
  /// ### Heap into Vec
  ///
  /// For `TinyVec::Heap(vec_data)`,
  ///   `.into()` will take `vec_data` without heap reallocation.
  ///
  /// ```
  /// use core::{
  ///   any::type_name_of_val as type_of, mem::size_of_val as mem_size_of,
  /// };
  /// use tinyvec::TinyVec;
  ///
  /// const fn from_heap<T: Default>(owned: Vec<T>) -> TinyVec<[T; 1]> {
  ///   TinyVec::Heap(owned)
  /// }
  ///
  /// let v = from_heap(vec![0u8; 128]);
  /// assert_eq!(v.len(), 128);
  /// assert_eq!(mem_size_of(&v), 24);
  /// assert!(type_of(&v).ends_with("TinyVec<[u8; 1]>"));
  ///
  /// let vec: Vec<_> = v.into();
  /// assert_eq!(mem_size_of(&vec), 24);
  /// assert!(type_of(&vec).ends_with("Vec<u8>"));
  /// ```
  #[inline]
  fn into(self) -> Vec<A::Item> {
    match self {
      Self::Heap(inner) => inner,
      Self::Inline(mut inner) => inner.drain_to_vec(),
    }
  }
}

/// Iterator for consuming an `TinyVec` and returning owned elements.
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub enum TinyVecIterator<A: Array> {
  #[allow(missing_docs)]
  Inline(ArrayVecIterator<A>),
  #[allow(missing_docs)]
  Heap(alloc::vec::IntoIter<A::Item>),
}

impl<A: Array> TinyVecIterator<A> {
  impl_mirrored! {
    type Mirror = TinyVecIterator;
    /// Returns the remaining items of this iterator as a slice.
    #[inline]
    #[must_use]
    pub fn as_slice(self: &Self) -> &[A::Item];
  }
}

impl<A: Array> FusedIterator for TinyVecIterator<A> {}

impl<A: Array> Iterator for TinyVecIterator<A> {
  type Item = A::Item;

  impl_mirrored! {
    type Mirror = TinyVecIterator;

    #[inline]
    fn next(self: &mut Self) -> Option<Self::Item>;

    #[inline(always)]
    #[must_use]
    fn size_hint(self: &Self) -> (usize, Option<usize>);

    #[inline(always)]
    fn count(self: Self) -> usize;

    #[inline]
    fn last(self: Self) -> Option<Self::Item>;

    #[inline]
    fn nth(self: &mut Self, n: usize) -> Option<A::Item>;
  }
}

impl<A: Array> DoubleEndedIterator for TinyVecIterator<A> {
  impl_mirrored! {
    type Mirror = TinyVecIterator;

    #[inline]
    fn next_back(self: &mut Self) -> Option<Self::Item>;

    #[inline]
    fn nth_back(self: &mut Self, n: usize) -> Option<Self::Item>;
  }
}

impl<A: Array> ExactSizeIterator for TinyVecIterator<A> {
  impl_mirrored! {
    type Mirror = TinyVecIterator;
    #[inline]
    fn len(self: &Self) -> usize;
  }
}

impl<A: Array> Debug for TinyVecIterator<A>
where
  A::Item: Debug,
{
  #[allow(clippy::missing_inline_in_public_items)]
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    f.debug_tuple("TinyVecIterator").field(&self.as_slice()).finish()
  }
}

impl<A: Array> IntoIterator for TinyVec<A> {
  type Item = A::Item;
  type IntoIter = TinyVecIterator<A>;
  #[inline(always)]
  fn into_iter(self) -> Self::IntoIter {
    match self {
      TinyVec::Inline(a) => TinyVecIterator::Inline(a.into_iter()),
      TinyVec::Heap(v) => TinyVecIterator::Heap(v.into_iter()),
    }
  }
}

impl<'a, A: Array> IntoIterator for &'a mut TinyVec<A> {
  type Item = &'a mut A::Item;
  type IntoIter = core::slice::IterMut<'a, A::Item>;
  #[inline(always)]
  fn into_iter(self) -> Self::IntoIter {
    self.iter_mut()
  }
}

impl<'a, A: Array> IntoIterator for &'a TinyVec<A> {
  type Item = &'a A::Item;
  type IntoIter = core::slice::Iter<'a, A::Item>;
  #[inline(always)]
  fn into_iter(self) -> Self::IntoIter {
    self.iter()
  }
}

impl<A: Array> PartialEq for TinyVec<A>
where
  A::Item: PartialEq,
{
  #[inline]
  fn eq(&self, other: &Self) -> bool {
    self.as_slice().eq(other.as_slice())
  }
}
impl<A: Array> Eq for TinyVec<A> where A::Item: Eq {}

impl<A: Array> PartialOrd for TinyVec<A>
where
  A::Item: PartialOrd,
{
  #[inline]
  fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
    self.as_slice().partial_cmp(other.as_slice())
  }
}
impl<A: Array> Ord for TinyVec<A>
where
  A::Item: Ord,
{
  #[inline]
  fn cmp(&self, other: &Self) -> core::cmp::Ordering {
    self.as_slice().cmp(other.as_slice())
  }
}

impl<A: Array> PartialEq<&A> for TinyVec<A>
where
  A::Item: PartialEq,
{
  #[inline]
  fn eq(&self, other: &&A) -> bool {
    self.as_slice().eq(other.as_slice())
  }
}

impl<A: Array> PartialEq<&[A::Item]> for TinyVec<A>
where
  A::Item: PartialEq,
{
  #[inline]
  fn eq(&self, other: &&[A::Item]) -> bool {
    self.as_slice().eq(*other)
  }
}

impl<A: Array> Hash for TinyVec<A>
where
  A::Item: Hash,
{
  #[inline]
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.as_slice().hash(state)
  }
}

// // // // // // // //
// Formatting impls
// // // // // // // //

impl<A: Array> Binary for TinyVec<A>
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

impl<A: Array> Debug for TinyVec<A>
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

impl<A: Array> Display for TinyVec<A>
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

impl<A: Array> LowerExp for TinyVec<A>
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

impl<A: Array> LowerHex for TinyVec<A>
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

impl<A: Array> Octal for TinyVec<A>
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

impl<A: Array> Pointer for TinyVec<A>
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

impl<A: Array> UpperExp for TinyVec<A>
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

impl<A: Array> UpperHex for TinyVec<A>
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

#[cfg(feature = "serde")]
#[cfg_attr(docs_rs, doc(cfg(feature = "alloc")))]
struct TinyVecVisitor<A: Array>(PhantomData<A>);

#[cfg(feature = "serde")]
impl<'de, A: Array> Visitor<'de> for TinyVecVisitor<A>
where
  A::Item: Deserialize<'de>,
{
  type Value = TinyVec<A>;

  fn expecting(
    &self, formatter: &mut core::fmt::Formatter,
  ) -> core::fmt::Result {
    formatter.write_str("a sequence")
  }

  fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
  where
    S: SeqAccess<'de>,
  {
    let mut new_tinyvec = match seq.size_hint() {
      Some(expected_size) => TinyVec::with_capacity(expected_size),
      None => Default::default(),
    };

    while let Some(value) = seq.next_element()? {
      new_tinyvec.push(value);
    }

    Ok(new_tinyvec)
  }
}
