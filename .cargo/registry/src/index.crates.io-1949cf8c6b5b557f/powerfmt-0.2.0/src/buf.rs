//! A buffer for constructing a string while avoiding heap allocation.

use core::hash::{Hash, Hasher};
use core::mem::MaybeUninit;
use core::{fmt, str};

use crate::smart_display::{FormatterOptions, Metadata, SmartDisplay};

/// A buffer for construct a string while avoiding heap allocation.
///
/// The only requirement is that the buffer is large enough to hold the formatted string.
pub struct WriteBuffer<const SIZE: usize> {
    buf: [MaybeUninit<u8>; SIZE],
    len: usize,
}

impl<const SIZE: usize> fmt::Debug for WriteBuffer<SIZE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DisplayBuffer")
            .field("buf", &self.as_str())
            .field("remaining_capacity", &self.remaining_capacity())
            .finish()
    }
}

impl<const SIZE: usize> WriteBuffer<SIZE> {
    /// Creates an empty buffer.
    pub const fn new() -> Self {
        Self {
            buf: maybe_uninit_uninit_array::<_, SIZE>(),
            len: 0,
        }
    }

    /// Obtain the contents of the buffer as a string.
    pub fn as_str(&self) -> &str {
        self
    }

    /// Determine how many bytes are remaining in the buffer.
    pub const fn remaining_capacity(&self) -> usize {
        SIZE - self.len
    }
}

impl<const SIZE: usize> Default for WriteBuffer<SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const LEFT_SIZE: usize, const RIGHT_SIZE: usize> PartialOrd<WriteBuffer<RIGHT_SIZE>>
    for WriteBuffer<LEFT_SIZE>
{
    fn partial_cmp(&self, other: &WriteBuffer<RIGHT_SIZE>) -> Option<core::cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl<const LEFT_SIZE: usize, const RIGHT_SIZE: usize> PartialEq<WriteBuffer<RIGHT_SIZE>>
    for WriteBuffer<LEFT_SIZE>
{
    fn eq(&self, other: &WriteBuffer<RIGHT_SIZE>) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<const SIZE: usize> Eq for WriteBuffer<SIZE> {}

impl<const SIZE: usize> Ord for WriteBuffer<SIZE> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl<const SIZE: usize> Hash for WriteBuffer<SIZE> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl<const SIZE: usize> AsRef<str> for WriteBuffer<SIZE> {
    fn as_ref(&self) -> &str {
        self
    }
}

impl<const SIZE: usize> AsRef<[u8]> for WriteBuffer<SIZE> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<const SIZE: usize> core::borrow::Borrow<str> for WriteBuffer<SIZE> {
    fn borrow(&self) -> &str {
        self
    }
}

impl<const SIZE: usize> core::ops::Deref for WriteBuffer<SIZE> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        // SAFETY: `buf` is only written to by the `fmt::Write::write_str` implementation which
        // writes a valid UTF-8 string to `buf` and correctly sets `len`.
        unsafe {
            let s = maybe_uninit_slice_assume_init_ref(&self.buf[..self.len]);
            str::from_utf8_unchecked(s)
        }
    }
}

impl<const SIZE: usize> fmt::Display for WriteBuffer<SIZE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}

impl<const SIZE: usize> SmartDisplay for WriteBuffer<SIZE> {
    type Metadata = ();

    fn metadata(&self, _: FormatterOptions) -> Metadata<'_, Self> {
        Metadata::new(self.len, self, ())
    }

    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self)
    }
}

impl<const SIZE: usize> fmt::Write for WriteBuffer<SIZE> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();

        if let Some(buf) = self.buf.get_mut(self.len..(self.len + bytes.len())) {
            maybe_uninit_write_slice(buf, bytes);
            self.len += bytes.len();
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}

/// Equivalent of [`MaybeUninit::uninit_array`] that compiles on stable.
#[must_use]
#[inline(always)]
const fn maybe_uninit_uninit_array<T, const N: usize>() -> [MaybeUninit<T>; N] {
    // SAFETY: An uninitialized `[MaybeUninit<_>; LEN]` is valid.
    unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() }
}

/// Equivalent of [`MaybeUninit::write_slice`] that compiles on stable.
fn maybe_uninit_write_slice<'a, T>(this: &'a mut [MaybeUninit<T>], src: &[T]) -> &'a mut [T]
where
    T: Copy,
{
    #[allow(trivial_casts)]
    // SAFETY: T and MaybeUninit<T> have the same layout
    let uninit_src = unsafe { &*(src as *const [T] as *const [MaybeUninit<T>]) };

    this.copy_from_slice(uninit_src);

    // SAFETY: Valid elements have just been copied into `this` so it is initialized
    unsafe { maybe_uninit_slice_assume_init_mut(this) }
}

/// Equivalent of [`MaybeUninit::slice_assume_init_mut`] that compiles on stable.
///
/// # Safety
///
/// See [`MaybeUninit::slice_assume_init_mut`](https://doc.rust-lang.org/stable/std/mem/union.MaybeUninit.html#method.slice_assume_init_mut).
#[inline(always)]
unsafe fn maybe_uninit_slice_assume_init_mut<T, U>(slice: &mut [MaybeUninit<T>]) -> &mut [U] {
    #[allow(trivial_casts)]
    // SAFETY: similar to safety notes for `slice_get_ref`, but we have a mutable reference which is
    // also guaranteed to be valid for writes.
    unsafe {
        &mut *(slice as *mut [MaybeUninit<T>] as *mut [U])
    }
}

/// Equivalent of [`MaybeUninit::slice_assume_init_ref`] that compiles on stable.
///
/// # Safety
///
/// See [`MaybeUninit::slice_assume_init_ref`](https://doc.rust-lang.org/stable/std/mem/union.MaybeUninit.html#method.slice_assume_init_ref).
#[inline(always)]
const unsafe fn maybe_uninit_slice_assume_init_ref<T>(slice: &[MaybeUninit<T>]) -> &[T] {
    #[allow(trivial_casts)]
    // SAFETY: casting `slice` to a `*const [T]` is safe since the caller guarantees that `slice` is
    // initialized, and `MaybeUninit` is guaranteed to have the same layout as `T`. The pointer
    // obtained is valid since it refers to memory owned by `slice` which is a reference and thus
    // guaranteed to be valid for reads.
    unsafe {
        &*(slice as *const [MaybeUninit<T>] as *const [T])
    }
}
