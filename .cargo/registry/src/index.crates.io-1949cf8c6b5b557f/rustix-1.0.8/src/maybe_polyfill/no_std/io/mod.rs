//! The following is derived from Rust's
//! library/std/src/sys/unix/io.rs
//! dca3f1b786efd27be3b325ed1e01e247aa589c3b.
//!
//! All code in this file is licensed MIT or Apache 2.0 at your option.

#![allow(unsafe_code)]
use crate::backend::c;
#[cfg(not(linux_raw))]
use c::size_t as __kernel_size_t;
use core::marker::PhantomData;
use core::slice;
#[cfg(linux_raw)]
use linux_raw_sys::general::__kernel_size_t;

/// <https://doc.rust-lang.org/stable/std/io/struct.IoSlice.html>
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct IoSlice<'a> {
    vec: c::iovec,
    _p: PhantomData<&'a [u8]>,
}

impl<'a> IoSlice<'a> {
    /// <https://doc.rust-lang.org/stable/std/io/struct.IoSlice.html#method.new>
    #[inline]
    pub fn new(buf: &'a [u8]) -> IoSlice<'a> {
        IoSlice {
            vec: c::iovec {
                iov_base: buf.as_ptr() as *mut u8 as *mut c::c_void,
                iov_len: buf.len() as _,
            },
            _p: PhantomData,
        }
    }

    /// <https://doc.rust-lang.org/stable/std/io/struct.IoSlice.html#method.advance>
    #[inline]
    pub fn advance(&mut self, n: usize) {
        if self.vec.iov_len < n as _ {
            panic!("advancing IoSlice beyond its length");
        }

        unsafe {
            // `__kernel_size_t` will always have the same size as `usize`, but it is a `u32` on
            // 32-bit platforms and `u64` on 64-bit platforms when using `linux_raw` backend
            self.vec.iov_len -= n as __kernel_size_t;
            self.vec.iov_base = self.vec.iov_base.add(n);
        }
    }

    /// <https://doc.rust-lang.org/stable/std/io/struct.IoSlice.html#method.as_slice>
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.vec.iov_base as *mut u8, self.vec.iov_len as usize) }
    }
}

/// <https://doc.rust-lang.org/stable/std/io/struct.IoSliceMut.html>
#[repr(transparent)]
pub struct IoSliceMut<'a> {
    vec: c::iovec,
    _p: PhantomData<&'a mut [u8]>,
}

impl<'a> IoSliceMut<'a> {
    /// <https://doc.rust-lang.org/stable/std/io/struct.IoSliceMut.html#method.new>
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> IoSliceMut<'a> {
        IoSliceMut {
            vec: c::iovec {
                iov_base: buf.as_mut_ptr() as *mut c::c_void,
                iov_len: buf.len() as _,
            },
            _p: PhantomData,
        }
    }

    /// <https://doc.rust-lang.org/stable/std/io/struct.IoSliceMut.html#method.advance>
    #[inline]
    pub fn advance(&mut self, n: usize) {
        if self.vec.iov_len < n as _ {
            panic!("advancing IoSliceMut beyond its length");
        }

        unsafe {
            // `__kernel_size_t` will always have the same size as `usize`, but it is a `u32` on
            // 32-bit platforms and `u64` on 64-bit platforms when using `linux_raw` backend
            self.vec.iov_len -= n as __kernel_size_t;
            self.vec.iov_base = self.vec.iov_base.add(n);
        }
    }

    /// <https://doc.rust-lang.org/stable/std/io/struct.IoSliceMut.html#method.as_slice>
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.vec.iov_base as *mut u8, self.vec.iov_len as usize) }
    }

    /// <https://doc.rust-lang.org/stable/std/io/struct.IoSliceMut.html#method.as_slice_mut>
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe {
            slice::from_raw_parts_mut(self.vec.iov_base as *mut u8, self.vec.iov_len as usize)
        }
    }
}
