//! Utilities for functions that return data via buffers.

#![allow(unsafe_code)]

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::slice;

/// A memory buffer that may be uninitialized.
///
/// There are three types that implement the `Buffer` trait, and the type you
/// use determines the return type of the functions that use it:
///
/// | If you pass a…           | You get back a… |
/// | ------------------------ | --------------- |
/// | `&mut [u8]`              | `usize`, indicating the number of elements initialized. |
/// | `&mut [MaybeUninit<u8>]` | `(&mut [u8], &mut [MaybeUninit<u8>])`, holding the initialized and uninitialized subslices. |
/// | [`SpareCapacity`]        | `usize`, indicating the number of elements initialized. And the `Vec` is extended. |
///
/// # Examples
///
/// Passing a `&mut [u8]`:
///
/// ```
/// # use rustix::io::read;
/// # fn example(fd: rustix::fd::BorrowedFd) -> rustix::io::Result<()> {
/// let mut buf = [0_u8; 64];
/// let nread = read(fd, &mut buf)?;
/// // `nread` is the number of bytes read.
/// # Ok(())
/// # }
/// ```
///
/// Passing a `&mut [MaybeUninit<u8>]`:
///
/// ```
/// # use rustix::io::read;
/// # use std::mem::MaybeUninit;
/// # fn example(fd: rustix::fd::BorrowedFd) -> rustix::io::Result<()> {
/// let mut buf = [MaybeUninit::<u8>::uninit(); 64];
/// let (init, uninit) = read(fd, &mut buf)?;
/// // `init` is a `&mut [u8]` with the initialized bytes.
/// // `uninit` is a `&mut [MaybeUninit<u8>]` with the remaining bytes.
/// # Ok(())
/// # }
/// ```
///
/// Passing a [`SpareCapacity`], via the [`spare_capacity`] helper function:
///
/// ```
/// # use rustix::io::read;
/// # use rustix::buffer::spare_capacity;
/// # fn example(fd: rustix::fd::BorrowedFd) -> rustix::io::Result<()> {
/// let mut buf = Vec::with_capacity(64);
/// let nread = read(fd, spare_capacity(&mut buf))?;
/// // `nread` is the number of bytes read.
/// // Also, `buf.len()` is now `nread` elements longer than it was before.
/// # Ok(())
/// # }
/// ```
///
/// # Guide to error messages
///
/// Sometimes code using `Buffer` can encounter non-obvious error messages.
/// Here are some we've encountered, along with ways to fix them.
///
/// If you see errors like
/// "cannot move out of `self` which is behind a mutable reference"
/// and
/// "move occurs because `x` has type `&mut [u8]`, which does not implement the `Copy` trait",
/// replace `x` with `&mut *x`. See `error_buffer_wrapper` in
/// examples/buffer_errors.rs.
///
/// If you see errors like
/// "type annotations needed"
/// and
/// "cannot infer type of the type parameter `Buf` declared on the function `read`",
/// you may need to change a `&mut []` to `&mut [0_u8; 0]`. See
/// `error_empty_slice` in examples/buffer_errors.rs.
///
/// If you see errors like
/// "the trait bound `[MaybeUninit<u8>; 1]: Buffer<u8>` is not satisfied",
/// add a `&mut` to pass the array by reference instead of by value. See
/// `error_array_by_value` in examples/buffer_errors.rs.
///
/// If you see errors like
/// "cannot move out of `x`, a captured variable in an `FnMut` closure",
/// try replacing `x` with `&mut *x`, or, if that doesn't work, try moving a
/// `let` into the closure body. See `error_retry_closure` and
/// `error_retry_indirect_closure` in examples/buffer_errors.rs.
///
/// If you see errors like
/// "captured variable cannot escape `FnMut` closure body",
/// use an explicit loop instead of `retry_on_intr`, assuming you're using
/// that. See `error_retry_closure_uninit` in examples/buffer_errors.rs.
pub trait Buffer<T>: private::Sealed<T> {}

// Implement `Buffer` for all the types that implement `Sealed`.
impl<T> Buffer<T> for &mut [T] {}
impl<T, const N: usize> Buffer<T> for &mut [T; N] {}
#[cfg(feature = "alloc")]
impl<T> Buffer<T> for &mut Vec<T> {}
impl<T> Buffer<T> for &mut [MaybeUninit<T>] {}
impl<T, const N: usize> Buffer<T> for &mut [MaybeUninit<T>; N] {}
#[cfg(feature = "alloc")]
impl<T> Buffer<T> for &mut Vec<MaybeUninit<T>> {}
#[cfg(feature = "alloc")]
impl<'a, T> Buffer<T> for SpareCapacity<'a, T> {}

impl<T> private::Sealed<T> for &mut [T] {
    type Output = usize;

    #[inline]
    fn parts_mut(&mut self) -> (*mut T, usize) {
        (self.as_mut_ptr(), self.len())
    }

    #[inline]
    unsafe fn assume_init(self, len: usize) -> Self::Output {
        len
    }
}

impl<T, const N: usize> private::Sealed<T> for &mut [T; N] {
    type Output = usize;

    #[inline]
    fn parts_mut(&mut self) -> (*mut T, usize) {
        (self.as_mut_ptr(), N)
    }

    #[inline]
    unsafe fn assume_init(self, len: usize) -> Self::Output {
        len
    }
}

// `Vec` implements `DerefMut` to `&mut [T]`, however it doesn't get
// auto-derefed in a `impl Buffer<u8>`, so we add this `impl` so that our users
// don't have to add an extra `*` in these situations.
#[cfg(feature = "alloc")]
impl<T> private::Sealed<T> for &mut Vec<T> {
    type Output = usize;

    #[inline]
    fn parts_mut(&mut self) -> (*mut T, usize) {
        (self.as_mut_ptr(), self.len())
    }

    #[inline]
    unsafe fn assume_init(self, len: usize) -> Self::Output {
        len
    }
}

impl<'a, T> private::Sealed<T> for &'a mut [MaybeUninit<T>] {
    type Output = (&'a mut [T], &'a mut [MaybeUninit<T>]);

    #[inline]
    fn parts_mut(&mut self) -> (*mut T, usize) {
        (self.as_mut_ptr().cast(), self.len())
    }

    #[inline]
    unsafe fn assume_init(self, len: usize) -> Self::Output {
        let (init, uninit) = self.split_at_mut(len);

        // SAFETY: The user asserts that the slice is now initialized.
        let init = slice::from_raw_parts_mut(init.as_mut_ptr().cast::<T>(), init.len());

        (init, uninit)
    }
}

impl<'a, T, const N: usize> private::Sealed<T> for &'a mut [MaybeUninit<T>; N] {
    type Output = (&'a mut [T], &'a mut [MaybeUninit<T>]);

    #[inline]
    fn parts_mut(&mut self) -> (*mut T, usize) {
        (self.as_mut_ptr().cast(), self.len())
    }

    #[inline]
    unsafe fn assume_init(self, len: usize) -> Self::Output {
        let (init, uninit) = self.split_at_mut(len);

        // SAFETY: The user asserts that the slice is now initialized.
        let init = slice::from_raw_parts_mut(init.as_mut_ptr().cast::<T>(), init.len());

        (init, uninit)
    }
}

#[cfg(feature = "alloc")]
impl<'a, T> private::Sealed<T> for &'a mut Vec<MaybeUninit<T>> {
    type Output = (&'a mut [T], &'a mut [MaybeUninit<T>]);

    #[inline]
    fn parts_mut(&mut self) -> (*mut T, usize) {
        (self.as_mut_ptr().cast(), self.len())
    }

    #[inline]
    unsafe fn assume_init(self, len: usize) -> Self::Output {
        let (init, uninit) = self.split_at_mut(len);

        // SAFETY: The user asserts that the slice is now initialized.
        let init = slice::from_raw_parts_mut(init.as_mut_ptr().cast::<T>(), init.len());

        (init, uninit)
    }
}

/// A type that implements [`Buffer`] by appending to a `Vec`, up to its
/// capacity.
///
/// To use this, use the [`spare_capacity`] function.
///
/// Because this uses the capacity, and never reallocates, the `Vec` should
/// have some non-empty spare capacity.
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct SpareCapacity<'a, T>(&'a mut Vec<T>);

/// Construct an [`SpareCapacity`], which implements [`Buffer`].
///
/// This wraps a `Vec` and uses the spare capacity of the `Vec` as the buffer
/// to receive data in, automatically calling `set_len` on the `Vec` to set the
/// length to include the received elements.
///
/// This uses the existing capacity, and never allocates, so the `Vec` should
/// have some non-empty spare capacity!
///
/// # Examples
///
/// ```
/// # fn test(input: rustix::fd::BorrowedFd) -> rustix::io::Result<()> {
/// use rustix::buffer::spare_capacity;
/// use rustix::io::{read, Errno};
///
/// let mut buf = Vec::with_capacity(1024);
/// match read(input, spare_capacity(&mut buf)) {
///     Ok(0) => { /* end of stream */ }
///     Ok(n) => { /* `buf` is now `n` bytes longer */ }
///     Err(Errno::INTR) => { /* `buf` is unmodified */ }
///     Err(e) => {
///         return Err(e);
///     }
/// }
///
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub fn spare_capacity<'a, T>(v: &'a mut Vec<T>) -> SpareCapacity<'a, T> {
    debug_assert_ne!(
        v.capacity(),
        0,
        "`extend` uses spare capacity, and never allocates new memory, so the `Vec` passed to it \
         should have some spare capacity."
    );

    SpareCapacity(v)
}

#[cfg(feature = "alloc")]
impl<'a, T> private::Sealed<T> for SpareCapacity<'a, T> {
    /// The mutated `Vec` reflects the number of bytes read. We also return
    /// this number, and a value of 0 indicates the end of the stream has
    /// been reached.
    type Output = usize;

    #[inline]
    fn parts_mut(&mut self) -> (*mut T, usize) {
        let spare = self.0.spare_capacity_mut();
        (spare.as_mut_ptr().cast(), spare.len())
    }

    #[inline]
    unsafe fn assume_init(self, len: usize) -> Self::Output {
        // We initialized `len` elements; extend the `Vec` to include them.
        self.0.set_len(self.0.len() + len);
        len
    }
}

mod private {
    pub trait Sealed<T> {
        /// The result of the process operation.
        type Output;

        /// Return a pointer and length for this buffer.
        ///
        /// The length is the number of elements of type `T`, not a number of
        /// bytes.
        ///
        /// It's tempting to have this return `&mut [MaybeUninit<T>]` instead,
        /// however that would require this function to be `unsafe`, because
        /// callers could use the `&mut [MaybeUninit<T>]` slice to set elements
        /// to `MaybeUninit::<T>::uninit()`, which would be a problem if `Self`
        /// is `&mut [T]` or similar.
        fn parts_mut(&mut self) -> (*mut T, usize);

        /// Convert a finished buffer pointer into its result.
        ///
        /// # Safety
        ///
        /// At least `len` elements of the buffer must now be initialized.
        #[must_use]
        unsafe fn assume_init(self, len: usize) -> Self::Output;
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn test_compilation() {
        use crate::io::read;
        use core::mem::MaybeUninit;

        // We need to obtain input stream, so open our own source file.
        let input = std::fs::File::open("src/buffer.rs").unwrap();

        let mut buf = vec![0_u8; 3];
        buf.reserve(32);
        let _x: usize = read(&input, spare_capacity(&mut buf)).unwrap();
        let _x: (&mut [u8], &mut [MaybeUninit<u8>]) =
            read(&input, buf.spare_capacity_mut()).unwrap();
        let _x: usize = read(&input, &mut buf).unwrap();
        let _x: usize = read(&input, &mut *buf).unwrap();
        let _x: usize = read(&input, &mut buf[..]).unwrap();
        let _x: usize = read(&input, &mut (*buf)[..]).unwrap();

        let mut buf = [0, 0, 0];
        let _x: usize = read(&input, &mut buf).unwrap();
        let _x: usize = read(&input, &mut buf[..]).unwrap();

        let mut buf = [
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
        ];
        let _x: (&mut [u8], &mut [MaybeUninit<u8>]) = read(&input, &mut buf).unwrap();
        let _x: (&mut [u8], &mut [MaybeUninit<u8>]) = read(&input, &mut buf[..]).unwrap();

        let mut buf = vec![
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
        ];
        let _x: (&mut [u8], &mut [MaybeUninit<u8>]) = read(&input, &mut buf).unwrap();
        let _x: (&mut [u8], &mut [MaybeUninit<u8>]) = read(&input, &mut buf[..]).unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn test_slice() {
        use crate::io::read;
        use std::io::{Seek, SeekFrom};

        // We need to obtain input stream with contents that we can compare
        // against, so open our own source file.
        let mut input = std::fs::File::open("src/buffer.rs").unwrap();

        let mut buf = [0_u8; 64];
        let nread = read(&input, &mut buf).unwrap();
        assert_eq!(nread, buf.len());
        assert_eq!(
            &buf[..58],
            b"//! Utilities for functions that return data via buffers.\n"
        );
        input.seek(SeekFrom::End(-1)).unwrap();
        let nread = read(&input, &mut buf).unwrap();
        assert_eq!(nread, 1);
        input.seek(SeekFrom::End(0)).unwrap();
        let nread = read(&input, &mut buf).unwrap();
        assert_eq!(nread, 0);
    }

    #[cfg(not(windows))]
    #[test]
    fn test_slice_uninit() {
        use crate::io::read;
        use core::mem::MaybeUninit;
        use std::io::{Seek, SeekFrom};

        // We need to obtain input stream with contents that we can compare
        // against, so open our own source file.
        let mut input = std::fs::File::open("src/buffer.rs").unwrap();

        let mut buf = [MaybeUninit::<u8>::uninit(); 64];
        let (init, uninit) = read(&input, &mut buf).unwrap();
        assert_eq!(uninit.len(), 0);
        assert_eq!(
            &init[..58],
            b"//! Utilities for functions that return data via buffers.\n"
        );
        assert_eq!(init.len(), buf.len());
        assert_eq!(
            unsafe { core::mem::transmute::<&mut [MaybeUninit<u8>], &mut [u8]>(&mut buf[..58]) },
            b"//! Utilities for functions that return data via buffers.\n"
        );
        input.seek(SeekFrom::End(-1)).unwrap();
        let (init, uninit) = read(&input, &mut buf).unwrap();
        assert_eq!(init.len(), 1);
        assert_eq!(uninit.len(), buf.len() - 1);
        input.seek(SeekFrom::End(0)).unwrap();
        let (init, uninit) = read(&input, &mut buf).unwrap();
        assert_eq!(init.len(), 0);
        assert_eq!(uninit.len(), buf.len());
    }

    #[cfg(not(windows))]
    #[test]
    fn test_spare_capacity() {
        use crate::io::read;
        use std::io::{Seek, SeekFrom};

        // We need to obtain input stream with contents that we can compare
        // against, so open our own source file.
        let mut input = std::fs::File::open("src/buffer.rs").unwrap();

        let mut buf = Vec::with_capacity(64);
        let nread = read(&input, spare_capacity(&mut buf)).unwrap();
        assert_eq!(nread, buf.capacity());
        assert_eq!(nread, buf.len());
        assert_eq!(
            &buf[..58],
            b"//! Utilities for functions that return data via buffers.\n"
        );
        buf.clear();
        input.seek(SeekFrom::End(-1)).unwrap();
        let nread = read(&input, spare_capacity(&mut buf)).unwrap();
        assert_eq!(nread, 1);
        assert_eq!(buf.len(), 1);
        buf.clear();
        input.seek(SeekFrom::End(0)).unwrap();
        let nread = read(&input, spare_capacity(&mut buf)).unwrap();
        assert_eq!(nread, 0);
        assert!(buf.is_empty());
    }
}
