use core::iter::FromIterator;
use core::mem::{self, ManuallyDrop};
use core::ops::{Deref, RangeBounds};
use core::ptr::NonNull;
use core::{cmp, fmt, hash, ptr, slice, usize};

use alloc::{
    alloc::{dealloc, Layout},
    borrow::Borrow,
    boxed::Box,
    string::String,
    vec::Vec,
};

use crate::buf::IntoIter;
#[allow(unused)]
use crate::loom::sync::atomic::AtomicMut;
use crate::loom::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use crate::{offset_from, Buf, BytesMut};

/// A cheaply cloneable and sliceable chunk of contiguous memory.
///
/// `Bytes` is an efficient container for storing and operating on contiguous
/// slices of memory. It is intended for use primarily in networking code, but
/// could have applications elsewhere as well.
///
/// `Bytes` values facilitate zero-copy network programming by allowing multiple
/// `Bytes` objects to point to the same underlying memory.
///
/// `Bytes` does not have a single implementation. It is an interface, whose
/// exact behavior is implemented through dynamic dispatch in several underlying
/// implementations of `Bytes`.
///
/// All `Bytes` implementations must fulfill the following requirements:
/// - They are cheaply cloneable and thereby shareable between an unlimited amount
///   of components, for example by modifying a reference count.
/// - Instances can be sliced to refer to a subset of the original buffer.
///
/// ```
/// use bytes::Bytes;
///
/// let mut mem = Bytes::from("Hello world");
/// let a = mem.slice(0..5);
///
/// assert_eq!(a, "Hello");
///
/// let b = mem.split_to(6);
///
/// assert_eq!(mem, "world");
/// assert_eq!(b, "Hello ");
/// ```
///
/// # Memory layout
///
/// The `Bytes` struct itself is fairly small, limited to 4 `usize` fields used
/// to track information about which segment of the underlying memory the
/// `Bytes` handle has access to.
///
/// `Bytes` keeps both a pointer to the shared state containing the full memory
/// slice and a pointer to the start of the region visible by the handle.
/// `Bytes` also tracks the length of its view into the memory.
///
/// # Sharing
///
/// `Bytes` contains a vtable, which allows implementations of `Bytes` to define
/// how sharing/cloning is implemented in detail.
/// When `Bytes::clone()` is called, `Bytes` will call the vtable function for
/// cloning the backing storage in order to share it behind multiple `Bytes`
/// instances.
///
/// For `Bytes` implementations which refer to constant memory (e.g. created
/// via `Bytes::from_static()`) the cloning implementation will be a no-op.
///
/// For `Bytes` implementations which point to a reference counted shared storage
/// (e.g. an `Arc<[u8]>`), sharing will be implemented by increasing the
/// reference count.
///
/// Due to this mechanism, multiple `Bytes` instances may point to the same
/// shared memory region.
/// Each `Bytes` instance can point to different sections within that
/// memory region, and `Bytes` instances may or may not have overlapping views
/// into the memory.
///
/// The following diagram visualizes a scenario where 2 `Bytes` instances make
/// use of an `Arc`-based backing storage, and provide access to different views:
///
/// ```text
///
///    Arc ptrs                   ┌─────────┐
///    ________________________ / │ Bytes 2 │
///   /                           └─────────┘
///  /          ┌───────────┐     |         |
/// |_________/ │  Bytes 1  │     |         |
/// |           └───────────┘     |         |
/// |           |           | ___/ data     | tail
/// |      data |      tail |/              |
/// v           v           v               v
/// ┌─────┬─────┬───────────┬───────────────┬─────┐
/// │ Arc │     │           │               │     │
/// └─────┴─────┴───────────┴───────────────┴─────┘
/// ```
pub struct Bytes {
    ptr: *const u8,
    len: usize,
    // inlined "trait object"
    data: AtomicPtr<()>,
    vtable: &'static Vtable,
}

pub(crate) struct Vtable {
    /// fn(data, ptr, len)
    pub clone: unsafe fn(&AtomicPtr<()>, *const u8, usize) -> Bytes,
    /// fn(data, ptr, len)
    ///
    /// takes `Bytes` to value
    pub to_vec: unsafe fn(&AtomicPtr<()>, *const u8, usize) -> Vec<u8>,
    pub to_mut: unsafe fn(&AtomicPtr<()>, *const u8, usize) -> BytesMut,
    /// fn(data)
    pub is_unique: unsafe fn(&AtomicPtr<()>) -> bool,
    /// fn(data, ptr, len)
    pub drop: unsafe fn(&mut AtomicPtr<()>, *const u8, usize),
}

impl Bytes {
    /// Creates a new empty `Bytes`.
    ///
    /// This will not allocate and the returned `Bytes` handle will be empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let b = Bytes::new();
    /// assert_eq!(&b[..], b"");
    /// ```
    #[inline]
    #[cfg(not(all(loom, test)))]
    pub const fn new() -> Self {
        // Make it a named const to work around
        // "unsizing casts are not allowed in const fn"
        const EMPTY: &[u8] = &[];
        Bytes::from_static(EMPTY)
    }

    /// Creates a new empty `Bytes`.
    #[cfg(all(loom, test))]
    pub fn new() -> Self {
        const EMPTY: &[u8] = &[];
        Bytes::from_static(EMPTY)
    }

    /// Creates a new `Bytes` from a static slice.
    ///
    /// The returned `Bytes` will point directly to the static slice. There is
    /// no allocating or copying.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let b = Bytes::from_static(b"hello");
    /// assert_eq!(&b[..], b"hello");
    /// ```
    #[inline]
    #[cfg(not(all(loom, test)))]
    pub const fn from_static(bytes: &'static [u8]) -> Self {
        Bytes {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
            data: AtomicPtr::new(ptr::null_mut()),
            vtable: &STATIC_VTABLE,
        }
    }

    /// Creates a new `Bytes` from a static slice.
    #[cfg(all(loom, test))]
    pub fn from_static(bytes: &'static [u8]) -> Self {
        Bytes {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
            data: AtomicPtr::new(ptr::null_mut()),
            vtable: &STATIC_VTABLE,
        }
    }

    /// Creates a new `Bytes` with length zero and the given pointer as the address.
    fn new_empty_with_ptr(ptr: *const u8) -> Self {
        debug_assert!(!ptr.is_null());

        // Detach this pointer's provenance from whichever allocation it came from, and reattach it
        // to the provenance of the fake ZST [u8;0] at the same address.
        let ptr = without_provenance(ptr as usize);

        Bytes {
            ptr,
            len: 0,
            data: AtomicPtr::new(ptr::null_mut()),
            vtable: &STATIC_VTABLE,
        }
    }

    /// Create [Bytes] with a buffer whose lifetime is controlled
    /// via an explicit owner.
    ///
    /// A common use case is to zero-copy construct from mapped memory.
    ///
    /// ```
    /// # struct File;
    /// #
    /// # impl File {
    /// #     pub fn open(_: &str) -> Result<Self, ()> {
    /// #         Ok(Self)
    /// #     }
    /// # }
    /// #
    /// # mod memmap2 {
    /// #     pub struct Mmap;
    /// #
    /// #     impl Mmap {
    /// #         pub unsafe fn map(_file: &super::File) -> Result<Self, ()> {
    /// #             Ok(Self)
    /// #         }
    /// #     }
    /// #
    /// #     impl AsRef<[u8]> for Mmap {
    /// #         fn as_ref(&self) -> &[u8] {
    /// #             b"buf"
    /// #         }
    /// #     }
    /// # }
    /// use bytes::Bytes;
    /// use memmap2::Mmap;
    ///
    /// # fn main() -> Result<(), ()> {
    /// let file = File::open("upload_bundle.tar.gz")?;
    /// let mmap = unsafe { Mmap::map(&file) }?;
    /// let b = Bytes::from_owner(mmap);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The `owner` will be transferred to the constructed [Bytes] object, which
    /// will ensure it is dropped once all remaining clones of the constructed
    /// object are dropped. The owner will then be responsible for dropping the
    /// specified region of memory as part of its [Drop] implementation.
    ///
    /// Note that converting [Bytes] constructed from an owner into a [BytesMut]
    /// will always create a deep copy of the buffer into newly allocated memory.
    pub fn from_owner<T>(owner: T) -> Self
    where
        T: AsRef<[u8]> + Send + 'static,
    {
        // Safety & Miri:
        // The ownership of `owner` is first transferred to the `Owned` wrapper and `Bytes` object.
        // This ensures that the owner is pinned in memory, allowing us to call `.as_ref()` safely
        // since the lifetime of the owner is controlled by the lifetime of the new `Bytes` object,
        // and the lifetime of the resulting borrowed `&[u8]` matches that of the owner.
        // Note that this remains safe so long as we only call `.as_ref()` once.
        //
        // There are some additional special considerations here:
        //   * We rely on Bytes's Drop impl to clean up memory should `.as_ref()` panic.
        //   * Setting the `ptr` and `len` on the bytes object last (after moving the owner to
        //     Bytes) allows Miri checks to pass since it avoids obtaining the `&[u8]` slice
        //     from a stack-owned Box.
        // More details on this: https://github.com/tokio-rs/bytes/pull/742/#discussion_r1813375863
        //                  and: https://github.com/tokio-rs/bytes/pull/742/#discussion_r1813316032

        let owned = Box::into_raw(Box::new(Owned {
            lifetime: OwnedLifetime {
                ref_cnt: AtomicUsize::new(1),
                drop: owned_box_and_drop::<T>,
            },
            owner,
        }));

        let mut ret = Bytes {
            ptr: NonNull::dangling().as_ptr(),
            len: 0,
            data: AtomicPtr::new(owned.cast()),
            vtable: &OWNED_VTABLE,
        };

        let buf = unsafe { &*owned }.owner.as_ref();
        ret.ptr = buf.as_ptr();
        ret.len = buf.len();

        ret
    }

    /// Returns the number of bytes contained in this `Bytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let b = Bytes::from(&b"hello"[..]);
    /// assert_eq!(b.len(), 5);
    /// ```
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the `Bytes` has a length of 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let b = Bytes::new();
    /// assert!(b.is_empty());
    /// ```
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns true if this is the only reference to the data and
    /// `Into<BytesMut>` would avoid cloning the underlying buffer.
    ///
    /// Always returns false if the data is backed by a [static slice](Bytes::from_static),
    /// or an [owner](Bytes::from_owner).
    ///
    /// The result of this method may be invalidated immediately if another
    /// thread clones this value while this is being called. Ensure you have
    /// unique access to this value (`&mut Bytes`) first if you need to be
    /// certain the result is valid (i.e. for safety reasons).
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let a = Bytes::from(vec![1, 2, 3]);
    /// assert!(a.is_unique());
    /// let b = a.clone();
    /// assert!(!a.is_unique());
    /// ```
    pub fn is_unique(&self) -> bool {
        unsafe { (self.vtable.is_unique)(&self.data) }
    }

    /// Creates `Bytes` instance from slice, by copying it.
    pub fn copy_from_slice(data: &[u8]) -> Self {
        data.to_vec().into()
    }

    /// Returns a slice of self for the provided range.
    ///
    /// This will increment the reference count for the underlying memory and
    /// return a new `Bytes` handle set to the slice.
    ///
    /// This operation is `O(1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let a = Bytes::from(&b"hello world"[..]);
    /// let b = a.slice(2..5);
    ///
    /// assert_eq!(&b[..], b"llo");
    /// ```
    ///
    /// # Panics
    ///
    /// Requires that `begin <= end` and `end <= self.len()`, otherwise slicing
    /// will panic.
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Self {
        use core::ops::Bound;

        let len = self.len();

        let begin = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n.checked_add(1).expect("out of range"),
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(&n) => n.checked_add(1).expect("out of range"),
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };

        assert!(
            begin <= end,
            "range start must not be greater than end: {:?} <= {:?}",
            begin,
            end,
        );
        assert!(
            end <= len,
            "range end out of bounds: {:?} <= {:?}",
            end,
            len,
        );

        if end == begin {
            return Bytes::new();
        }

        let mut ret = self.clone();

        ret.len = end - begin;
        ret.ptr = unsafe { ret.ptr.add(begin) };

        ret
    }

    /// Returns a slice of self that is equivalent to the given `subset`.
    ///
    /// When processing a `Bytes` buffer with other tools, one often gets a
    /// `&[u8]` which is in fact a slice of the `Bytes`, i.e. a subset of it.
    /// This function turns that `&[u8]` into another `Bytes`, as if one had
    /// called `self.slice()` with the offsets that correspond to `subset`.
    ///
    /// This operation is `O(1)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let bytes = Bytes::from(&b"012345678"[..]);
    /// let as_slice = bytes.as_ref();
    /// let subset = &as_slice[2..6];
    /// let subslice = bytes.slice_ref(&subset);
    /// assert_eq!(&subslice[..], b"2345");
    /// ```
    ///
    /// # Panics
    ///
    /// Requires that the given `sub` slice is in fact contained within the
    /// `Bytes` buffer; otherwise this function will panic.
    pub fn slice_ref(&self, subset: &[u8]) -> Self {
        // Empty slice and empty Bytes may have their pointers reset
        // so explicitly allow empty slice to be a subslice of any slice.
        if subset.is_empty() {
            return Bytes::new();
        }

        let bytes_p = self.as_ptr() as usize;
        let bytes_len = self.len();

        let sub_p = subset.as_ptr() as usize;
        let sub_len = subset.len();

        assert!(
            sub_p >= bytes_p,
            "subset pointer ({:p}) is smaller than self pointer ({:p})",
            subset.as_ptr(),
            self.as_ptr(),
        );
        assert!(
            sub_p + sub_len <= bytes_p + bytes_len,
            "subset is out of bounds: self = ({:p}, {}), subset = ({:p}, {})",
            self.as_ptr(),
            bytes_len,
            subset.as_ptr(),
            sub_len,
        );

        let sub_offset = sub_p - bytes_p;

        self.slice(sub_offset..(sub_offset + sub_len))
    }

    /// Splits the bytes into two at the given index.
    ///
    /// Afterwards `self` contains elements `[0, at)`, and the returned `Bytes`
    /// contains elements `[at, len)`. It's guaranteed that the memory does not
    /// move, that is, the address of `self` does not change, and the address of
    /// the returned slice is `at` bytes after that.
    ///
    /// This is an `O(1)` operation that just increases the reference count and
    /// sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let mut a = Bytes::from(&b"hello world"[..]);
    /// let b = a.split_off(5);
    ///
    /// assert_eq!(&a[..], b"hello");
    /// assert_eq!(&b[..], b" world");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    #[must_use = "consider Bytes::truncate if you don't need the other half"]
    pub fn split_off(&mut self, at: usize) -> Self {
        if at == self.len() {
            return Bytes::new_empty_with_ptr(self.ptr.wrapping_add(at));
        }

        if at == 0 {
            return mem::replace(self, Bytes::new_empty_with_ptr(self.ptr));
        }

        assert!(
            at <= self.len(),
            "split_off out of bounds: {:?} <= {:?}",
            at,
            self.len(),
        );

        let mut ret = self.clone();

        self.len = at;

        unsafe { ret.inc_start(at) };

        ret
    }

    /// Splits the bytes into two at the given index.
    ///
    /// Afterwards `self` contains elements `[at, len)`, and the returned
    /// `Bytes` contains elements `[0, at)`.
    ///
    /// This is an `O(1)` operation that just increases the reference count and
    /// sets a few indices.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let mut a = Bytes::from(&b"hello world"[..]);
    /// let b = a.split_to(5);
    ///
    /// assert_eq!(&a[..], b" world");
    /// assert_eq!(&b[..], b"hello");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    #[must_use = "consider Bytes::advance if you don't need the other half"]
    pub fn split_to(&mut self, at: usize) -> Self {
        if at == self.len() {
            let end_ptr = self.ptr.wrapping_add(at);
            return mem::replace(self, Bytes::new_empty_with_ptr(end_ptr));
        }

        if at == 0 {
            return Bytes::new_empty_with_ptr(self.ptr);
        }

        assert!(
            at <= self.len(),
            "split_to out of bounds: {:?} <= {:?}",
            at,
            self.len(),
        );

        let mut ret = self.clone();

        unsafe { self.inc_start(at) };

        ret.len = at;
        ret
    }

    /// Shortens the buffer, keeping the first `len` bytes and dropping the
    /// rest.
    ///
    /// If `len` is greater than the buffer's current length, this has no
    /// effect.
    ///
    /// The [split_off](`Self::split_off()`) method can emulate `truncate`, but this causes the
    /// excess bytes to be returned instead of dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let mut buf = Bytes::from(&b"hello world"[..]);
    /// buf.truncate(5);
    /// assert_eq!(buf, b"hello"[..]);
    /// ```
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        if len < self.len {
            // The Vec "promotable" vtables do not store the capacity,
            // so we cannot truncate while using this repr. We *have* to
            // promote using `split_off` so the capacity can be stored.
            if self.vtable as *const Vtable == &PROMOTABLE_EVEN_VTABLE
                || self.vtable as *const Vtable == &PROMOTABLE_ODD_VTABLE
            {
                drop(self.split_off(len));
            } else {
                self.len = len;
            }
        }
    }

    /// Clears the buffer, removing all data.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    ///
    /// let mut buf = Bytes::from(&b"hello world"[..]);
    /// buf.clear();
    /// assert!(buf.is_empty());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// Try to convert self into `BytesMut`.
    ///
    /// If `self` is unique for the entire original buffer, this will succeed
    /// and return a `BytesMut` with the contents of `self` without copying.
    /// If `self` is not unique for the entire original buffer, this will fail
    /// and return self.
    ///
    /// This will also always fail if the buffer was constructed via either
    /// [from_owner](Bytes::from_owner) or [from_static](Bytes::from_static).
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Bytes, BytesMut};
    ///
    /// let bytes = Bytes::from(b"hello".to_vec());
    /// assert_eq!(bytes.try_into_mut(), Ok(BytesMut::from(&b"hello"[..])));
    /// ```
    pub fn try_into_mut(self) -> Result<BytesMut, Bytes> {
        if self.is_unique() {
            Ok(self.into())
        } else {
            Err(self)
        }
    }

    #[inline]
    pub(crate) unsafe fn with_vtable(
        ptr: *const u8,
        len: usize,
        data: AtomicPtr<()>,
        vtable: &'static Vtable,
    ) -> Bytes {
        Bytes {
            ptr,
            len,
            data,
            vtable,
        }
    }

    // private

    #[inline]
    fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }

    #[inline]
    unsafe fn inc_start(&mut self, by: usize) {
        // should already be asserted, but debug assert for tests
        debug_assert!(self.len >= by, "internal: inc_start out of bounds");
        self.len -= by;
        self.ptr = self.ptr.add(by);
    }
}

// Vtable must enforce this behavior
unsafe impl Send for Bytes {}
unsafe impl Sync for Bytes {}

impl Drop for Bytes {
    #[inline]
    fn drop(&mut self) {
        unsafe { (self.vtable.drop)(&mut self.data, self.ptr, self.len) }
    }
}

impl Clone for Bytes {
    #[inline]
    fn clone(&self) -> Bytes {
        unsafe { (self.vtable.clone)(&self.data, self.ptr, self.len) }
    }
}

impl Buf for Bytes {
    #[inline]
    fn remaining(&self) -> usize {
        self.len()
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        self.as_slice()
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        assert!(
            cnt <= self.len(),
            "cannot advance past `remaining`: {:?} <= {:?}",
            cnt,
            self.len(),
        );

        unsafe {
            self.inc_start(cnt);
        }
    }

    fn copy_to_bytes(&mut self, len: usize) -> Self {
        self.split_to(len)
    }
}

impl Deref for Bytes {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl AsRef<[u8]> for Bytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl hash::Hash for Bytes {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        self.as_slice().hash(state);
    }
}

impl Borrow<[u8]> for Bytes {
    fn borrow(&self) -> &[u8] {
        self.as_slice()
    }
}

impl IntoIterator for Bytes {
    type Item = u8;
    type IntoIter = IntoIter<Bytes>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self)
    }
}

impl<'a> IntoIterator for &'a Bytes {
    type Item = &'a u8;
    type IntoIter = core::slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl FromIterator<u8> for Bytes {
    fn from_iter<T: IntoIterator<Item = u8>>(into_iter: T) -> Self {
        Vec::from_iter(into_iter).into()
    }
}

// impl Eq

impl PartialEq for Bytes {
    fn eq(&self, other: &Bytes) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl PartialOrd for Bytes {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl Ord for Bytes {
    fn cmp(&self, other: &Bytes) -> cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl Eq for Bytes {}

impl PartialEq<[u8]> for Bytes {
    fn eq(&self, other: &[u8]) -> bool {
        self.as_slice() == other
    }
}

impl PartialOrd<[u8]> for Bytes {
    fn partial_cmp(&self, other: &[u8]) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(other)
    }
}

impl PartialEq<Bytes> for [u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for [u8] {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other)
    }
}

impl PartialEq<str> for Bytes {
    fn eq(&self, other: &str) -> bool {
        self.as_slice() == other.as_bytes()
    }
}

impl PartialOrd<str> for Bytes {
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_bytes())
    }
}

impl PartialEq<Bytes> for str {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for str {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self.as_bytes(), other)
    }
}

impl PartialEq<Vec<u8>> for Bytes {
    fn eq(&self, other: &Vec<u8>) -> bool {
        *self == other[..]
    }
}

impl PartialOrd<Vec<u8>> for Bytes {
    fn partial_cmp(&self, other: &Vec<u8>) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(&other[..])
    }
}

impl PartialEq<Bytes> for Vec<u8> {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for Vec<u8> {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other)
    }
}

impl PartialEq<String> for Bytes {
    fn eq(&self, other: &String) -> bool {
        *self == other[..]
    }
}

impl PartialOrd<String> for Bytes {
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_bytes())
    }
}

impl PartialEq<Bytes> for String {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for String {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self.as_bytes(), other)
    }
}

impl PartialEq<Bytes> for &[u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for &[u8] {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other)
    }
}

impl PartialEq<Bytes> for &str {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for &str {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self.as_bytes(), other)
    }
}

impl<'a, T: ?Sized> PartialEq<&'a T> for Bytes
where
    Bytes: PartialEq<T>,
{
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}

impl<'a, T: ?Sized> PartialOrd<&'a T> for Bytes
where
    Bytes: PartialOrd<T>,
{
    fn partial_cmp(&self, other: &&'a T) -> Option<cmp::Ordering> {
        self.partial_cmp(&**other)
    }
}

// impl From

impl Default for Bytes {
    #[inline]
    fn default() -> Bytes {
        Bytes::new()
    }
}

impl From<&'static [u8]> for Bytes {
    fn from(slice: &'static [u8]) -> Bytes {
        Bytes::from_static(slice)
    }
}

impl From<&'static str> for Bytes {
    fn from(slice: &'static str) -> Bytes {
        Bytes::from_static(slice.as_bytes())
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(vec: Vec<u8>) -> Bytes {
        let mut vec = ManuallyDrop::new(vec);
        let ptr = vec.as_mut_ptr();
        let len = vec.len();
        let cap = vec.capacity();

        // Avoid an extra allocation if possible.
        if len == cap {
            let vec = ManuallyDrop::into_inner(vec);
            return Bytes::from(vec.into_boxed_slice());
        }

        let shared = Box::new(Shared {
            buf: ptr,
            cap,
            ref_cnt: AtomicUsize::new(1),
        });

        let shared = Box::into_raw(shared);
        // The pointer should be aligned, so this assert should
        // always succeed.
        debug_assert!(
            0 == (shared as usize & KIND_MASK),
            "internal: Box<Shared> should have an aligned pointer",
        );
        Bytes {
            ptr,
            len,
            data: AtomicPtr::new(shared as _),
            vtable: &SHARED_VTABLE,
        }
    }
}

impl From<Box<[u8]>> for Bytes {
    fn from(slice: Box<[u8]>) -> Bytes {
        // Box<[u8]> doesn't contain a heap allocation for empty slices,
        // so the pointer isn't aligned enough for the KIND_VEC stashing to
        // work.
        if slice.is_empty() {
            return Bytes::new();
        }

        let len = slice.len();
        let ptr = Box::into_raw(slice) as *mut u8;

        if ptr as usize & 0x1 == 0 {
            let data = ptr_map(ptr, |addr| addr | KIND_VEC);
            Bytes {
                ptr,
                len,
                data: AtomicPtr::new(data.cast()),
                vtable: &PROMOTABLE_EVEN_VTABLE,
            }
        } else {
            Bytes {
                ptr,
                len,
                data: AtomicPtr::new(ptr.cast()),
                vtable: &PROMOTABLE_ODD_VTABLE,
            }
        }
    }
}

impl From<Bytes> for BytesMut {
    /// Convert self into `BytesMut`.
    ///
    /// If `bytes` is unique for the entire original buffer, this will return a
    /// `BytesMut` with the contents of `bytes` without copying.
    /// If `bytes` is not unique for the entire original buffer, this will make
    /// a copy of `bytes` subset of the original buffer in a new `BytesMut`.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::{Bytes, BytesMut};
    ///
    /// let bytes = Bytes::from(b"hello".to_vec());
    /// assert_eq!(BytesMut::from(bytes), BytesMut::from(&b"hello"[..]));
    /// ```
    fn from(bytes: Bytes) -> Self {
        let bytes = ManuallyDrop::new(bytes);
        unsafe { (bytes.vtable.to_mut)(&bytes.data, bytes.ptr, bytes.len) }
    }
}

impl From<String> for Bytes {
    fn from(s: String) -> Bytes {
        Bytes::from(s.into_bytes())
    }
}

impl From<Bytes> for Vec<u8> {
    fn from(bytes: Bytes) -> Vec<u8> {
        let bytes = ManuallyDrop::new(bytes);
        unsafe { (bytes.vtable.to_vec)(&bytes.data, bytes.ptr, bytes.len) }
    }
}

// ===== impl Vtable =====

impl fmt::Debug for Vtable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vtable")
            .field("clone", &(self.clone as *const ()))
            .field("drop", &(self.drop as *const ()))
            .finish()
    }
}

// ===== impl StaticVtable =====

const STATIC_VTABLE: Vtable = Vtable {
    clone: static_clone,
    to_vec: static_to_vec,
    to_mut: static_to_mut,
    is_unique: static_is_unique,
    drop: static_drop,
};

unsafe fn static_clone(_: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    let slice = slice::from_raw_parts(ptr, len);
    Bytes::from_static(slice)
}

unsafe fn static_to_vec(_: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
    let slice = slice::from_raw_parts(ptr, len);
    slice.to_vec()
}

unsafe fn static_to_mut(_: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BytesMut {
    let slice = slice::from_raw_parts(ptr, len);
    BytesMut::from(slice)
}

fn static_is_unique(_: &AtomicPtr<()>) -> bool {
    false
}

unsafe fn static_drop(_: &mut AtomicPtr<()>, _: *const u8, _: usize) {
    // nothing to drop for &'static [u8]
}

// ===== impl OwnedVtable =====

#[repr(C)]
struct OwnedLifetime {
    ref_cnt: AtomicUsize,
    drop: unsafe fn(*mut ()),
}

#[repr(C)]
struct Owned<T> {
    lifetime: OwnedLifetime,
    owner: T,
}

unsafe fn owned_box_and_drop<T>(ptr: *mut ()) {
    let b: Box<Owned<T>> = Box::from_raw(ptr as _);
    drop(b);
}

unsafe fn owned_clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    let owned = data.load(Ordering::Relaxed);
    let ref_cnt = &(*owned.cast::<OwnedLifetime>()).ref_cnt;
    let old_cnt = ref_cnt.fetch_add(1, Ordering::Relaxed);
    if old_cnt > usize::MAX >> 1 {
        crate::abort()
    }

    Bytes {
        ptr,
        len,
        data: AtomicPtr::new(owned as _),
        vtable: &OWNED_VTABLE,
    }
}

unsafe fn owned_to_vec(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
    let slice = slice::from_raw_parts(ptr, len);
    let vec = slice.to_vec();
    owned_drop_impl(data.load(Ordering::Relaxed));
    vec
}

unsafe fn owned_to_mut(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BytesMut {
    BytesMut::from_vec(owned_to_vec(data, ptr, len))
}

unsafe fn owned_is_unique(_data: &AtomicPtr<()>) -> bool {
    false
}

unsafe fn owned_drop_impl(owned: *mut ()) {
    let lifetime = owned.cast::<OwnedLifetime>();
    let ref_cnt = &(*lifetime).ref_cnt;

    let old_cnt = ref_cnt.fetch_sub(1, Ordering::Release);
    debug_assert!(
        old_cnt > 0 && old_cnt <= usize::MAX >> 1,
        "expected non-zero refcount and no underflow"
    );
    if old_cnt != 1 {
        return;
    }
    ref_cnt.load(Ordering::Acquire);

    let drop_fn = &(*lifetime).drop;
    drop_fn(owned)
}

unsafe fn owned_drop(data: &mut AtomicPtr<()>, _ptr: *const u8, _len: usize) {
    let owned = data.load(Ordering::Relaxed);
    owned_drop_impl(owned);
}

static OWNED_VTABLE: Vtable = Vtable {
    clone: owned_clone,
    to_vec: owned_to_vec,
    to_mut: owned_to_mut,
    is_unique: owned_is_unique,
    drop: owned_drop,
};

// ===== impl PromotableVtable =====

static PROMOTABLE_EVEN_VTABLE: Vtable = Vtable {
    clone: promotable_even_clone,
    to_vec: promotable_even_to_vec,
    to_mut: promotable_even_to_mut,
    is_unique: promotable_is_unique,
    drop: promotable_even_drop,
};

static PROMOTABLE_ODD_VTABLE: Vtable = Vtable {
    clone: promotable_odd_clone,
    to_vec: promotable_odd_to_vec,
    to_mut: promotable_odd_to_mut,
    is_unique: promotable_is_unique,
    drop: promotable_odd_drop,
};

unsafe fn promotable_even_clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    let shared = data.load(Ordering::Acquire);
    let kind = shared as usize & KIND_MASK;

    if kind == KIND_ARC {
        shallow_clone_arc(shared.cast(), ptr, len)
    } else {
        debug_assert_eq!(kind, KIND_VEC);
        let buf = ptr_map(shared.cast(), |addr| addr & !KIND_MASK);
        shallow_clone_vec(data, shared, buf, ptr, len)
    }
}

unsafe fn promotable_to_vec(
    data: &AtomicPtr<()>,
    ptr: *const u8,
    len: usize,
    f: fn(*mut ()) -> *mut u8,
) -> Vec<u8> {
    let shared = data.load(Ordering::Acquire);
    let kind = shared as usize & KIND_MASK;

    if kind == KIND_ARC {
        shared_to_vec_impl(shared.cast(), ptr, len)
    } else {
        // If Bytes holds a Vec, then the offset must be 0.
        debug_assert_eq!(kind, KIND_VEC);

        let buf = f(shared);

        let cap = offset_from(ptr, buf) + len;

        // Copy back buffer
        ptr::copy(ptr, buf, len);

        Vec::from_raw_parts(buf, len, cap)
    }
}

unsafe fn promotable_to_mut(
    data: &AtomicPtr<()>,
    ptr: *const u8,
    len: usize,
    f: fn(*mut ()) -> *mut u8,
) -> BytesMut {
    let shared = data.load(Ordering::Acquire);
    let kind = shared as usize & KIND_MASK;

    if kind == KIND_ARC {
        shared_to_mut_impl(shared.cast(), ptr, len)
    } else {
        // KIND_VEC is a view of an underlying buffer at a certain offset.
        // The ptr + len always represents the end of that buffer.
        // Before truncating it, it is first promoted to KIND_ARC.
        // Thus, we can safely reconstruct a Vec from it without leaking memory.
        debug_assert_eq!(kind, KIND_VEC);

        let buf = f(shared);
        let off = offset_from(ptr, buf);
        let cap = off + len;
        let v = Vec::from_raw_parts(buf, cap, cap);

        let mut b = BytesMut::from_vec(v);
        b.advance_unchecked(off);
        b
    }
}

unsafe fn promotable_even_to_vec(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
    promotable_to_vec(data, ptr, len, |shared| {
        ptr_map(shared.cast(), |addr| addr & !KIND_MASK)
    })
}

unsafe fn promotable_even_to_mut(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BytesMut {
    promotable_to_mut(data, ptr, len, |shared| {
        ptr_map(shared.cast(), |addr| addr & !KIND_MASK)
    })
}

unsafe fn promotable_even_drop(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) {
    data.with_mut(|shared| {
        let shared = *shared;
        let kind = shared as usize & KIND_MASK;

        if kind == KIND_ARC {
            release_shared(shared.cast());
        } else {
            debug_assert_eq!(kind, KIND_VEC);
            let buf = ptr_map(shared.cast(), |addr| addr & !KIND_MASK);
            free_boxed_slice(buf, ptr, len);
        }
    });
}

unsafe fn promotable_odd_clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    let shared = data.load(Ordering::Acquire);
    let kind = shared as usize & KIND_MASK;

    if kind == KIND_ARC {
        shallow_clone_arc(shared as _, ptr, len)
    } else {
        debug_assert_eq!(kind, KIND_VEC);
        shallow_clone_vec(data, shared, shared.cast(), ptr, len)
    }
}

unsafe fn promotable_odd_to_vec(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
    promotable_to_vec(data, ptr, len, |shared| shared.cast())
}

unsafe fn promotable_odd_to_mut(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BytesMut {
    promotable_to_mut(data, ptr, len, |shared| shared.cast())
}

unsafe fn promotable_odd_drop(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) {
    data.with_mut(|shared| {
        let shared = *shared;
        let kind = shared as usize & KIND_MASK;

        if kind == KIND_ARC {
            release_shared(shared.cast());
        } else {
            debug_assert_eq!(kind, KIND_VEC);

            free_boxed_slice(shared.cast(), ptr, len);
        }
    });
}

unsafe fn promotable_is_unique(data: &AtomicPtr<()>) -> bool {
    let shared = data.load(Ordering::Acquire);
    let kind = shared as usize & KIND_MASK;

    if kind == KIND_ARC {
        let ref_cnt = (*shared.cast::<Shared>()).ref_cnt.load(Ordering::Relaxed);
        ref_cnt == 1
    } else {
        true
    }
}

unsafe fn free_boxed_slice(buf: *mut u8, offset: *const u8, len: usize) {
    let cap = offset_from(offset, buf) + len;
    dealloc(buf, Layout::from_size_align(cap, 1).unwrap())
}

// ===== impl SharedVtable =====

struct Shared {
    // Holds arguments to dealloc upon Drop, but otherwise doesn't use them
    buf: *mut u8,
    cap: usize,
    ref_cnt: AtomicUsize,
}

impl Drop for Shared {
    fn drop(&mut self) {
        unsafe { dealloc(self.buf, Layout::from_size_align(self.cap, 1).unwrap()) }
    }
}

// Assert that the alignment of `Shared` is divisible by 2.
// This is a necessary invariant since we depend on allocating `Shared` a
// shared object to implicitly carry the `KIND_ARC` flag in its pointer.
// This flag is set when the LSB is 0.
const _: [(); 0 - mem::align_of::<Shared>() % 2] = []; // Assert that the alignment of `Shared` is divisible by 2.

static SHARED_VTABLE: Vtable = Vtable {
    clone: shared_clone,
    to_vec: shared_to_vec,
    to_mut: shared_to_mut,
    is_unique: shared_is_unique,
    drop: shared_drop,
};

const KIND_ARC: usize = 0b0;
const KIND_VEC: usize = 0b1;
const KIND_MASK: usize = 0b1;

unsafe fn shared_clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    let shared = data.load(Ordering::Relaxed);
    shallow_clone_arc(shared as _, ptr, len)
}

unsafe fn shared_to_vec_impl(shared: *mut Shared, ptr: *const u8, len: usize) -> Vec<u8> {
    // Check that the ref_cnt is 1 (unique).
    //
    // If it is unique, then it is set to 0 with AcqRel fence for the same
    // reason in release_shared.
    //
    // Otherwise, we take the other branch and call release_shared.
    if (*shared)
        .ref_cnt
        .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
    {
        // Deallocate the `Shared` instance without running its destructor.
        let shared = *Box::from_raw(shared);
        let shared = ManuallyDrop::new(shared);
        let buf = shared.buf;
        let cap = shared.cap;

        // Copy back buffer
        ptr::copy(ptr, buf, len);

        Vec::from_raw_parts(buf, len, cap)
    } else {
        let v = slice::from_raw_parts(ptr, len).to_vec();
        release_shared(shared);
        v
    }
}

unsafe fn shared_to_vec(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
    shared_to_vec_impl(data.load(Ordering::Relaxed).cast(), ptr, len)
}

unsafe fn shared_to_mut_impl(shared: *mut Shared, ptr: *const u8, len: usize) -> BytesMut {
    // The goal is to check if the current handle is the only handle
    // that currently has access to the buffer. This is done by
    // checking if the `ref_cnt` is currently 1.
    //
    // The `Acquire` ordering synchronizes with the `Release` as
    // part of the `fetch_sub` in `release_shared`. The `fetch_sub`
    // operation guarantees that any mutations done in other threads
    // are ordered before the `ref_cnt` is decremented. As such,
    // this `Acquire` will guarantee that those mutations are
    // visible to the current thread.
    //
    // Otherwise, we take the other branch, copy the data and call `release_shared`.
    if (*shared).ref_cnt.load(Ordering::Acquire) == 1 {
        // Deallocate the `Shared` instance without running its destructor.
        let shared = *Box::from_raw(shared);
        let shared = ManuallyDrop::new(shared);
        let buf = shared.buf;
        let cap = shared.cap;

        // Rebuild Vec
        let off = offset_from(ptr, buf);
        let v = Vec::from_raw_parts(buf, len + off, cap);

        let mut b = BytesMut::from_vec(v);
        b.advance_unchecked(off);
        b
    } else {
        // Copy the data from Shared in a new Vec, then release it
        let v = slice::from_raw_parts(ptr, len).to_vec();
        release_shared(shared);
        BytesMut::from_vec(v)
    }
}

unsafe fn shared_to_mut(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BytesMut {
    shared_to_mut_impl(data.load(Ordering::Relaxed).cast(), ptr, len)
}

pub(crate) unsafe fn shared_is_unique(data: &AtomicPtr<()>) -> bool {
    let shared = data.load(Ordering::Acquire);
    let ref_cnt = (*shared.cast::<Shared>()).ref_cnt.load(Ordering::Relaxed);
    ref_cnt == 1
}

unsafe fn shared_drop(data: &mut AtomicPtr<()>, _ptr: *const u8, _len: usize) {
    data.with_mut(|shared| {
        release_shared(shared.cast());
    });
}

unsafe fn shallow_clone_arc(shared: *mut Shared, ptr: *const u8, len: usize) -> Bytes {
    let old_size = (*shared).ref_cnt.fetch_add(1, Ordering::Relaxed);

    if old_size > usize::MAX >> 1 {
        crate::abort();
    }

    Bytes {
        ptr,
        len,
        data: AtomicPtr::new(shared as _),
        vtable: &SHARED_VTABLE,
    }
}

#[cold]
unsafe fn shallow_clone_vec(
    atom: &AtomicPtr<()>,
    ptr: *const (),
    buf: *mut u8,
    offset: *const u8,
    len: usize,
) -> Bytes {
    // If the buffer is still tracked in a `Vec<u8>`. It is time to
    // promote the vec to an `Arc`. This could potentially be called
    // concurrently, so some care must be taken.

    // First, allocate a new `Shared` instance containing the
    // `Vec` fields. It's important to note that `ptr`, `len`,
    // and `cap` cannot be mutated without having `&mut self`.
    // This means that these fields will not be concurrently
    // updated and since the buffer hasn't been promoted to an
    // `Arc`, those three fields still are the components of the
    // vector.
    let shared = Box::new(Shared {
        buf,
        cap: offset_from(offset, buf) + len,
        // Initialize refcount to 2. One for this reference, and one
        // for the new clone that will be returned from
        // `shallow_clone`.
        ref_cnt: AtomicUsize::new(2),
    });

    let shared = Box::into_raw(shared);

    // The pointer should be aligned, so this assert should
    // always succeed.
    debug_assert!(
        0 == (shared as usize & KIND_MASK),
        "internal: Box<Shared> should have an aligned pointer",
    );

    // Try compare & swapping the pointer into the `arc` field.
    // `Release` is used synchronize with other threads that
    // will load the `arc` field.
    //
    // If the `compare_exchange` fails, then the thread lost the
    // race to promote the buffer to shared. The `Acquire`
    // ordering will synchronize with the `compare_exchange`
    // that happened in the other thread and the `Shared`
    // pointed to by `actual` will be visible.
    match atom.compare_exchange(ptr as _, shared as _, Ordering::AcqRel, Ordering::Acquire) {
        Ok(actual) => {
            debug_assert!(actual as usize == ptr as usize);
            // The upgrade was successful, the new handle can be
            // returned.
            Bytes {
                ptr: offset,
                len,
                data: AtomicPtr::new(shared as _),
                vtable: &SHARED_VTABLE,
            }
        }
        Err(actual) => {
            // The upgrade failed, a concurrent clone happened. Release
            // the allocation that was made in this thread, it will not
            // be needed.
            let shared = Box::from_raw(shared);
            mem::forget(*shared);

            // Buffer already promoted to shared storage, so increment ref
            // count.
            shallow_clone_arc(actual as _, offset, len)
        }
    }
}

unsafe fn release_shared(ptr: *mut Shared) {
    // `Shared` storage... follow the drop steps from Arc.
    if (*ptr).ref_cnt.fetch_sub(1, Ordering::Release) != 1 {
        return;
    }

    // This fence is needed to prevent reordering of use of the data and
    // deletion of the data.  Because it is marked `Release`, the decreasing
    // of the reference count synchronizes with this `Acquire` fence. This
    // means that use of the data happens before decreasing the reference
    // count, which happens before this fence, which happens before the
    // deletion of the data.
    //
    // As explained in the [Boost documentation][1],
    //
    // > It is important to enforce any possible access to the object in one
    // > thread (through an existing reference) to *happen before* deleting
    // > the object in a different thread. This is achieved by a "release"
    // > operation after dropping a reference (any access to the object
    // > through this reference must obviously happened before), and an
    // > "acquire" operation before deleting the object.
    //
    // [1]: (www.boost.org/doc/libs/1_55_0/doc/html/atomic/usage_examples.html)
    //
    // Thread sanitizer does not support atomic fences. Use an atomic load
    // instead.
    (*ptr).ref_cnt.load(Ordering::Acquire);

    // Drop the data
    drop(Box::from_raw(ptr));
}

// Ideally we would always use this version of `ptr_map` since it is strict
// provenance compatible, but it results in worse codegen. We will however still
// use it on miri because it gives better diagnostics for people who test bytes
// code with miri.
//
// See https://github.com/tokio-rs/bytes/pull/545 for more info.
#[cfg(miri)]
fn ptr_map<F>(ptr: *mut u8, f: F) -> *mut u8
where
    F: FnOnce(usize) -> usize,
{
    let old_addr = ptr as usize;
    let new_addr = f(old_addr);
    let diff = new_addr.wrapping_sub(old_addr);
    ptr.wrapping_add(diff)
}

#[cfg(not(miri))]
fn ptr_map<F>(ptr: *mut u8, f: F) -> *mut u8
where
    F: FnOnce(usize) -> usize,
{
    let old_addr = ptr as usize;
    let new_addr = f(old_addr);
    new_addr as *mut u8
}

fn without_provenance(ptr: usize) -> *const u8 {
    core::ptr::null::<u8>().wrapping_add(ptr)
}

// compile-fails

/// ```compile_fail
/// use bytes::Bytes;
/// #[deny(unused_must_use)]
/// {
///     let mut b1 = Bytes::from("hello world");
///     b1.split_to(6);
/// }
/// ```
fn _split_to_must_use() {}

/// ```compile_fail
/// use bytes::Bytes;
/// #[deny(unused_must_use)]
/// {
///     let mut b1 = Bytes::from("hello world");
///     b1.split_off(6);
/// }
/// ```
fn _split_off_must_use() {}

// fuzz tests
#[cfg(all(test, loom))]
mod fuzz {
    use loom::sync::Arc;
    use loom::thread;

    use super::Bytes;
    #[test]
    fn bytes_cloning_vec() {
        loom::model(|| {
            let a = Bytes::from(b"abcdefgh".to_vec());
            let addr = a.as_ptr() as usize;

            // test the Bytes::clone is Sync by putting it in an Arc
            let a1 = Arc::new(a);
            let a2 = a1.clone();

            let t1 = thread::spawn(move || {
                let b: Bytes = (*a1).clone();
                assert_eq!(b.as_ptr() as usize, addr);
            });

            let t2 = thread::spawn(move || {
                let b: Bytes = (*a2).clone();
                assert_eq!(b.as_ptr() as usize, addr);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }
}
